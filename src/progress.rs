use std::borrow::Cow;

use indicatif::{MultiProgress, ProgressBar, ProgressFinish, ProgressStyle};

pub struct ProgressDisplay {
	style: ProgressStyle,
	finished_style: ProgressStyle,
	progress_bars: MultiProgress,
	total_progress: ProgressBar,
}

impl ProgressDisplay {
	pub fn new(total_progress: u64) -> Self {
		let total_style = ProgressStyle::with_template("{prefix} ({binary_bytes_per_sec} | {eta} remaining):  [{wide_bar:.blue/blue}]  {percent}% ").unwrap()
			.progress_chars("##-");
		let style = ProgressStyle::with_template("{prefix}:  [{wide_bar:.yellow/yellow}]  {percent}% ").unwrap()
			.progress_chars("##-");
		let finished_style = ProgressStyle::with_template("{prefix}  [{wide_bar:.green}]  {percent}% ").unwrap()
			.progress_chars("##-");
		
		let progress_bars = MultiProgress::new();
		let total_progress = ProgressBar::new(total_progress)
			.with_finish(ProgressFinish::AndLeave)
			.with_prefix("Total")
			.with_style(total_style);
		progress_bars.add(total_progress.clone());
		
		Self {
			style,
			finished_style,
			progress_bars,
			total_progress,
		}
	}
	
	pub fn new_tracker(&self, label: impl Into<Cow<'static, str>>, total_progress: u64) -> ProgressTracker {
		let progress = ProgressBar::new(total_progress)
			.with_finish(ProgressFinish::AndLeave)
			.with_style(self.style.clone())
			.with_prefix(label);
		self.progress_bars.insert_from_back(1, progress.clone());
		
		ProgressTracker {
			display: &self,
			progress,
			total_progress,
		}
	}
}

pub struct ProgressTracker<'a> {
	display: &'a ProgressDisplay,
	progress: ProgressBar,
	total_progress: u64,
}

impl<'a> ProgressTracker<'a> {
	pub fn advance(&self, amount: u64) {
		self.display.total_progress.inc(amount);
		self.progress.inc(amount);
		
		if self.progress.position() == self.total_progress {
			self.progress.set_style(self.display.finished_style.clone());
			self.progress.finish();
		}
	}
}
