//! A dead simple MPD shuffler written in pure Rust.
//!
//! This program keeps track of the songs it has already played and will not
//! play them again until every song in your MPD database has been played.
//!
//! This shuffler does not interfere with your queue except when it is empty.
//! You can continue queueing songs as normal and this program will not add anything
//! until the queue is completely empty and there is nothing left to play.

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use mpd::{Client, Idle, Subsystem};
use rand::{seq::SliceRandom, thread_rng};
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, EnvFilter};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[macro_use]
extern crate tracing;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(short = 'H', long, default_value = "0.0.0.0")]
    pub host: String,
    #[clap(short, long, default_value_t = 6600)]
    pub port: u16,
}

fn main() -> Result<()> {
    // parse args before initialization so we don't set up logging if we don't need to
    let cli = Cli::parse();

    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_file(false)
                .with_line_number(true),
        )
        .with(
            EnvFilter::builder()
                .with_default_directive(concat!(env!("CARGO_PKG_NAME"), "=info").parse().unwrap())
                .from_env_lossy(),
        )
        .with(ErrorLayer::default())
        .init();

    color_eyre::install()?;

    let uri = format!("{}:{}", cli.host, cli.port);
    trace!("using uri: {}", uri);

    let mut attempts = 0;
    let mut last_attempt_at = Instant::now();
    /// If an attempt lasted longer than this duration, assume the previous attempt was actually successful and reset the counter.
    const ATTEMPT_INTERVAL: Duration = Duration::from_secs(10);

    let mut already_played = HashSet::<String>::new();

    while attempts < 3 {
        if let Err(err) = event_loop(&uri, &mut already_played) {
            error!("error in event loop: {}", err);
            if last_attempt_at.elapsed() > ATTEMPT_INTERVAL {
                debug!("attempt interval elapsed, resetting attempt counter");
                attempts = 0;
            } else {
                attempts += 1;
            }

            last_attempt_at = Instant::now();
        } else {
            unreachable!("event loop should never return Ok")
        }
    }

    error!("failed to run event loop 3 times, exiting");

    Ok(())
}

/// The inner event loop. If this function returns,
/// the main function will attempt to reconnect and restart it.
///
/// If it fails to rerun this function 3 times, the program will exit.
#[instrument(skip(already_played))]
fn event_loop(uri: &str, already_played: &mut HashSet<String>) -> Result<()> {
    trace!("connecting");
    let mut client = Client::connect(uri)?;
    info!("connected");

    loop {
        trace!("subsystems changed, checking status");
        let status = client.status()?;
        trace!("status: {:?}", status);

        let active = status.nextsong.is_none() && status.song.is_none();

        if active {
            trace!("queue empty, queuing next song");
            queue_next(&mut client, already_played, status.queue_len)?;
            info!("waiting for queue to finish...");
        }

        trace!("watching Queue and Player subsystems");
        client.wait(&[Subsystem::Queue, Subsystem::Player])?;
    }
}

/// Queue a random song. "Queue" in this context means push the song
/// into the playlist and switch to it.
///
/// Will only play songs which are not in the `already_played` set.
/// If there are no more songs left, the `already_played` set will be cleared.
#[instrument(skip(client, already_played))]
fn queue_next(
    client: &mut Client,
    already_played: &mut HashSet<String>,
    queue_len: u32,
) -> Result<()> {
    // listall only returns the song paths which is all we care about
    let songs = client.listall()?;
    trace!("received {} songs from MPD", songs.len());

    if songs.len() == 0 {
        return Err(eyre!("no songs in library"));
    }

    let songs = songs
        .into_iter()
        .filter_map(|song| {
            if !already_played.contains(&song.file) {
                Some(song)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    trace!("{} songs left to play", songs.len());

    if songs.len() == 0 {
        warn!("no songs left to play, resetting");
        already_played.clear();
        return queue_next(client, already_played, queue_len);
    }

    let next = songs
        .choose(&mut thread_rng())
        .ok_or_else(|| eyre!("no songs to choose from"))?;

    info!("playing {}", next.file);

    client.push(next)?;

    // status was captured before we added the song
    // and queue is zero-indexed, so we can use the old
    // length as the new position
    client.switch(queue_len)?;

    already_played.insert(next.file.clone());

    trace!("already played: {already_played:?}");

    Ok(())
}
