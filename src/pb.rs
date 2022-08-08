use std::{cmp::min, thread, time::Duration};

use crate::downloader::DownloadObserver;
use indicatif::{ProgressBar, ProgressStyle};

pub struct ProgressManager {
    pbs: Vec<ProgressBar>,
}

impl DownloadObserver for ProgressManager {
    fn on_download_start(&mut self, part: u8, total_size: u64) {
        println!("on download start for {}: {:?}", part, total_size);
    }

    fn on_progress(&mut self, part: u8, progress: u64) {
        println!("on progress for {}: {:?}", part, progress);
    }

    fn on_download_end(&mut self, part: u8) {
        println!("on download end for {}", part);
    }

    fn on_message(&mut self, msg: &str) {
        println!("on message {}", msg);
    }
}

impl ProgressManager {
    pub fn new() -> Self {
        Self { pbs: Vec::new() }
    }
}

pub fn test_show_pb() {
    let mut downloaded: u32 = 0;
    let total_size: u32 = 243 * 1024 * 1024;

    let pb = ProgressBar::new(total_size as u64);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap());
    // .progress_chars("#>-"));

    while downloaded < total_size {
        let new = 750 * 1024;
        downloaded = min(downloaded + new, total_size);

        pb.set_position(downloaded as u64);
        // pb.inc(new as u64);
        thread::sleep(Duration::from_millis(100));
    }

    pb.finish_with_message("downloaded")
}
