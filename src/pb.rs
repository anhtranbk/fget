use std::{cmp::min, thread, time::Duration};

use crate::downloader::DownloadObserver;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub struct ProgressManager {
    m: MultiProgress,
    pbs: Vec<ProgressBar>,
}

impl DownloadObserver for ProgressManager {
    fn on_download_start(&mut self, idx: u8, len: u64) {
        if let Some(pb) = self.pbs.get_mut(idx as usize) {
            pb.set_length(len);
        }
    }

    fn on_progress(&mut self, idx: u8, pos: u64) {
        if let Some(pb) = self.pbs.get_mut(idx as usize) {
            pb.set_position(pos);
        }
    }

    fn on_download_end(&mut self, idx: u8) {
        if let Some(pb) = self.pbs.get_mut(idx as usize) {
            pb.finish_with_message(format!("part {} downloaded", idx));
        }
    }

    fn on_init(&mut self, len: usize) {
        for i in 0..len {
            self.pbs.push(self.m.insert(i, new_progress_bar(0)));
        }
    }
}

impl ProgressManager {
    pub fn new() -> Self {
        Self {
            pbs: vec![],
            m: MultiProgress::new(),
        }
    }
}

fn new_progress_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len as u64);
    pb.set_style(
        ProgressStyle::with_template(concat!(
            "{spinner:.green} ",
            "[{elapsed_precise}] ",
            "[{wide_bar:.cyan/blue}] ",
            "{bytes}/{total_bytes} ",
            "({binary_bytes_per_sec:^12} ",
            "{eta:>3})"
        ))
        .unwrap()
        .progress_chars("#>-"),
    );

    pb
}

#[allow(dead_code)]
pub fn test_show_pb() {
    let len: u64 = 243 * 1024 * 1024;
    let mut n = 4;
    let mut pbs = vec![];
    let mut vpos = vec![0u64; n];

    let m = MultiProgress::new();
    for i in 0..n {
        let pb = m.insert(i, new_progress_bar(len));
        pbs.push(pb);
    }

    while n > 0 {
        for i in 0..4usize {
            let new = 750 * 1024;
            let pos = min(vpos[i] + new, len);

            pbs[i].set_position(pos);
            vpos[i] = pos;
            thread::sleep(Duration::from_millis(10));
        }

        if vpos[0] == len {
            n -= 1;
        }
    }
    for i in 0..4 {
        pbs[i].finish_with_message(format!("part {} downloaded", i));
    }
}
