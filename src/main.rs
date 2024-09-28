//! A dead simple MPD shuffler written in pure Rust.
//!
//! This program keeps track of the songs it has already played and will not play them again until
//! every song in your MPD database has been played.
//!
//! This shuffler does not interfere with your queue except when it is empty. You can continue
//! queueing songs as normal and this program will not add anything until the queue is completely
//! empty and there is nothing left to play.

use std::time::{Duration, Instant};

use async_recursion::async_recursion;
use clap::Parser;
use color_eyre::eyre::{bail, eyre, Result};
use filter::Filter;
use mpd_client::{
    client::{ConnectionEvent, Subsystem},
    commands::{self as cmd, SongPosition},
    responses as res, Client,
};
use rand::{rngs::ThreadRng, seq::SliceRandom, thread_rng};
use state::AppState;
use tokio::net::TcpStream;
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, EnvFilter};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[macro_use]
extern crate tracing;

mod filter;
mod state;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// The hostname of the MPD server
    #[clap(short = 'H', long, default_value = "0.0.0.0")]
    pub host: String,

    /// The port of the MPD server
    #[clap(short = 'P', long, default_value_t = 6600)]
    pub port: u16,

    /// The number of additional songs to keep in the playlist after the current song
    ///
    /// This is required for crossfade to work
    #[clap(short = 'b', long, default_value_t = 0)]
    pub num_buffer: u32,

    /// Don't keep track of which songs have been played
    #[clap(short, long)]
    pub no_tracking: bool,

    /// Persist the state of the program across restarts
    ///
    /// This is useful if you have a massive music library and you want to listen to each song once
    /// over the course of a few days instead of in one sitting.
    #[clap(short, long)]
    pub persist: bool,

    /// Only play songs which contain any of these strings in their titles. Can be specified
    /// multiple times
    #[clap(short, long)]
    pub filter: Vec<String>,
}

struct AppContext {
    pub uri: String,
    pub num_buffer: u32,
    pub state: Option<AppState>,
    pub rng: ThreadRng,
    pub filters: Vec<Filter>,
    // we want inverted filters to be separate because they are applied after the normal filters
    pub inverted_filters: Vec<Filter>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // parse args before initialization so we don't set up logging if we don't need to
    let cli = Cli::parse();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_file(true)
                .with_line_number(true)
                .compact(),
        )
        .with(
            EnvFilter::builder()
                .with_default_directive(concat!(env!("CARGO_PKG_NAME"), "=info").parse().unwrap())
                .from_env_lossy(),
        )
        .with(ErrorLayer::default())
        .init();

    color_eyre::install()?;

    trace!("parsed args: {:?}", cli);

    let uri = format!("{}:{}", cli.host, cli.port);
    trace!("using uri: {}", uri);

    let mut attempts = 0;
    let mut last_attempt_at = Instant::now();
    /// If an attempt lasted longer than this duration, assume the previous attempt was actually
    /// successful and reset the counter.
    const ATTEMPT_INTERVAL: Duration = Duration::from_secs(30);

    let rng = thread_rng();

    let mut filters = Vec::<Filter>::new();

    for filter in cli.filter {
        match filter.parse() {
            Ok(f) => filters.push(f),
            Err(err) => {
                error!("failed to parse filter \"{filter}\": {err:?}");
            }
        }
    }

    trace!("filters: {filters:?}");

    let (inverted_filters, filters) = filters.into_iter().partition(Filter::is_inverted);
    trace!("inverted filters: {inverted_filters:?}");

    let mut ctx = AppContext {
        uri,
        num_buffer: cli.num_buffer,
        state: if cli.no_tracking {
            None
        } else {
            Some(AppState::load(cli.persist).await?)
        },
        rng,
        filters,
        inverted_filters,
    };

    while attempts < 3 {
        if let Err(err) = event_loop(&mut ctx).await {
            error!("error in event loop: {}", err);
            if last_attempt_at.elapsed() > ATTEMPT_INTERVAL {
                debug!("attempt interval elapsed, resetting attempt counter");
                attempts = 0;
            } else {
                attempts += 1;
            }

            last_attempt_at = Instant::now();

            std::thread::sleep(Duration::from_secs(2));
        } else {
            unreachable!("event loop should never return Ok")
        }
    }

    error!("failed to run event loop 3 times, exiting");

    Ok(())
}

/// The inner event loop. If this function returns, the main function will attempt to reconnect and
/// restart it.
///
/// If it fails to rerun this function 3 times, the program will exit.
#[instrument(skip_all)]
async fn event_loop(ctx: &mut AppContext) -> Result<()> {
    trace!("connecting");
    let connection = TcpStream::connect(&ctx.uri).await?;
    let (mut client, mut events) = Client::connect(connection).await?;
    info!("connected");

    loop {
        debug!("subsystems changed, checking status");
        let status = client.command(cmd::Status).await?;
        trace!("status: {:?}", status);

        let active = is_active(ctx, &status);
        trace!("activity status: {:?}", active);

        match active {
            ActivityStatus::NotActive => {
                debug!("not active, doing nothing");
            }
            ActivityStatus::Active(n, play_first) => {
                trace!("active, adding {} songs to queue", n);
                let switch_to = if play_first {
                    Some(status.playlist_length)
                } else {
                    None
                };
                queue_next(&mut client, ctx, switch_to).await?;

                for _ in 0..n - 1 {
                    queue_next(&mut client, ctx, None).await?;
                }
            }
        }

        trace!("watching Queue and Player subsystems");
        loop {
            let Some(event) = events.next().await else {
                bail!("connection closed gracefully");
            };

            match event {
                ConnectionEvent::ConnectionClosed(ev) => {
                    bail!("connection closed: {:?}", ev);
                }
                ConnectionEvent::SubsystemChange(Subsystem::Queue | Subsystem::Player) => {
                    break;
                }
                _ => {
                    trace!("ignoring event: {:?}", event);
                }
            }
        }
    }
}

#[derive(Debug)]
enum ActivityStatus {
    /// We do not need to add anymore songs to the queue
    NotActive,
    /// We need to add N more songs to the queue. Second argument is whether or not we should play
    /// the first song.
    Active(u32, bool),
}

#[inline]
#[instrument(skip(ctx))]
fn is_active(ctx: &mut AppContext, status: &res::Status) -> ActivityStatus {
    if status.next_song.is_none() && status.current_song.is_none() {
        trace!("no next song and no current song");
        ActivityStatus::Active(1 + ctx.num_buffer, true)
    } else if ctx.num_buffer != 0 {
        // calculate how many songs remain after the current song in the queue. the `queue_len`
        // returned by MPD will keep growing if consume mode is off so we need to subtract the
        // current song's position from the total length

        // can't be zero because the first condition would have returned
        let len = status.playlist_length;

        let Some((current, _)) = &status.current_song else {
            trace!("no current song, but there is a next song");
            // this block is likely unreachable, but it is here for completeness. this block will
            // likely be caught because if status.song is None then status.nextsong will probably
            // also be None, which the very first condition catches
            return ActivityStatus::Active(1 + ctx.num_buffer, true);
        };

        let current = current.0;
        // SAFETY: it should be impossible for the current song to be past the last song in the
        // queue and since len > 0 we know that current <= len > 0
        let remaining = (len - current - 1) as u32;
        if remaining == 0 {
            trace!("no songs remaining in queue");
            // we are playing a song and it is the only song in the queue, and we know there are
            // more songs to play because we would have returned if num_buffer == 0
            return ActivityStatus::Active(ctx.num_buffer, false);
        }

        // if there are less songs remaining than the buffer size, we are active
        if remaining < ctx.num_buffer {
            trace!(
                "{} songs remaining in queue, less than buffer size of {}",
                remaining,
                ctx.num_buffer
            );
            ActivityStatus::Active(remaining, false)
        } else {
            trace!(
                "{} songs remaining in queue, more than or equal to the buffer size of {}",
                remaining,
                ctx.num_buffer
            );
            // otherwise we are not active
            ActivityStatus::NotActive
        }
    } else {
        trace!("there is a song playing or up next and num_buffer is 0");
        // there is either a current song or next song, and num_buffer is 0, so we are not active
        return ActivityStatus::NotActive;
    }
}

/// Queue a random song. "Queue" in this context means push the song into the playlist and switch to
/// it.
///
/// Will only play songs which are not in the `already_played` set. If there are no more songs left,
/// the `already_played` set will be cleared.
///
/// If queue_len is Some(_), switch to that song
#[instrument(skip_all)]
#[async_recursion(?Send)]
async fn queue_next(
    client: &mut Client,
    ctx: &mut AppContext,
    switch_to: Option<usize>,
) -> Result<()> {
    let AppContext {
        uri: _,
        num_buffer: _,
        state,
        rng,
        filters,
        inverted_filters,
    } = ctx;

    // listall only returns the song paths which isn't enough information if we want to filter
    let mut songs = client.command(cmd::ListAllIn::root()).await?;
    trace!("received {} songs from MPD", songs.len());

    if songs.is_empty() {
        return Err(eyre!("no songs in library"));
    }

    if !filters.is_empty() {
        songs = songs
            .into_iter()
            .filter(|song| filters.iter().any(|filter| filter.matches(song)))
            .collect::<Vec<_>>();
        debug!("{} songs left after filtering", songs.len());

        if songs.is_empty() {
            // this is an error because we haven't filtered out already played tracks which means
            // the filters match nothing and probably never will
            return Err(eyre!("no songs left after filtering"));
        }
    }

    if !inverted_filters.is_empty() {
        songs = songs
            .into_iter()
            .filter(|song| inverted_filters.iter().all(|filter| !filter.matches(song)))
            .collect::<Vec<_>>();
        debug!("{} songs left after inverted filtering", songs.len());

        if songs.is_empty() {
            // this is an error because we haven't filtered out already played tracks which means
            // the filters match nothing and probably never will
            return Err(eyre!("no songs left after inverted filtering"));
        }
    }

    if let Some(state) = state {
        songs = songs
            .into_iter()
            .filter_map(|song| {
                if state.has_been_played(&song) {
                    None
                } else {
                    Some(song)
                }
            })
            .collect::<Vec<_>>();
        info!("{} songs left to play", songs.len());

        if songs.is_empty() {
            warn!("no songs left to play, resetting");
            state.clear();
            return queue_next(client, ctx, switch_to).await;
        }
    }

    let next = songs
        .choose(rng)
        .ok_or_else(|| eyre!("no songs to choose from"))?;

    info!("playing {}", next.url);

    client.command(cmd::Add::uri(&next.url)).await?;

    // status was captured before we added the song and queue is zero-indexed, so we can use the old
    // length as the new position
    if let Some(switch_to) = switch_to {
        trace!("switching to song {}", switch_to);
        client
            .command(cmd::Play::song(SongPosition(switch_to)))
            .await?;
    }

    if let Some(state) = state {
        state.mark_as_played(next);
        state.save().await?;

        trace!("state: {state:?}");
    }

    Ok(())
}
