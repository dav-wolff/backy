use std::borrow::Cow;

use indicatif::{MultiProgress, ProgressBar, ProgressFinish, ProgressStyle};

pub trait ProgressDisplay: Sync {
	fn new_tracker(&self, label: Cow<'static, str>, total_progress: u64) -> Box<dyn ProgressTracker + '_>;
}

pub trait ProgressTracker {
	fn advance(&self, amount: u64);
}

pub struct NoopProgressDisplay;

impl ProgressDisplay for NoopProgressDisplay {
	fn new_tracker(&self, _label: Cow<'static, str>, _total_progress: u64) -> Box<dyn ProgressTracker + '_> {
		Box::new(NoopTracker)
	}
}

struct NoopTracker;

impl ProgressTracker for NoopTracker {
	fn advance(&self, _amount: u64) { }
}

pub struct TerminalProgressDisplay {
	style: ProgressStyle,
	finished_style: ProgressStyle,
	progress_bars: MultiProgress,
	total_progress: ProgressBar,
}

impl TerminalProgressDisplay {
	pub fn new(total_progress: u64) -> Self {
		let total_style = ProgressStyle::with_template("{prefix} ({binary_bytes_per_sec} | {eta} remaining):  [{wide_bar:.blue/blue}]  {percent}% ").expect("should be valid")
			.progress_chars("##-");
		let style = ProgressStyle::with_template("{prefix}:  [{wide_bar:.yellow/yellow}]  {percent}% ").expect("should be valid")
			.progress_chars("##-");
		let finished_style = ProgressStyle::with_template("{prefix}  [{wide_bar:.green}]  {percent}% ").expect("should be valid")
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
}

impl ProgressDisplay for TerminalProgressDisplay {
	fn new_tracker(&self, label: Cow<'static, str>, total_progress: u64) -> Box<dyn ProgressTracker + '_> {
		let progress = ProgressBar::new(total_progress)
			.with_finish(ProgressFinish::AndLeave)
			.with_style(self.style.clone())
			.with_prefix(label);
		self.progress_bars.insert_from_back(1, progress.clone());
		progress.tick();
		
		Box::new(TerminalTracker {
			display: self,
			progress,
			total_progress,
		})
	}
}

struct TerminalTracker<'a> {
	display: &'a TerminalProgressDisplay,
	progress: ProgressBar,
	total_progress: u64,
}

impl ProgressTracker for TerminalTracker<'_> {
	fn advance(&self, amount: u64) {
		self.display.total_progress.inc(amount);
		self.progress.inc(amount);
		
		if self.progress.position() == self.total_progress {
			self.progress.set_style(self.display.finished_style.clone());
			self.progress.finish();
		}
	}
}
