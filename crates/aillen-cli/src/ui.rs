use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::io::{stdout, Write};
use std::time::{Instant, Duration};
use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
    style::Print,
};

const UI_WIDTH: usize = 80;
const UI_HEIGHT: usize = 43; // Exactly 43 rows total

const TRACK_ROWS: usize = 30; // Rows 0..30
const MASTER_START_ROW: usize = 31; // Rows 31..42

const MASTER_COLS: usize = 60; // 60 cols for master, 20 cols for scope
const TRACK_WAVEFORM_ROWS: usize = 28; // Rows 2..29 for track vertical waveforms
const MASTER_WAVEFORM_COLS: usize = 58; // Length for master horizontal waveform

const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(15);

/// Applies soft gain boost & square-root compression for prominent visual waveform scaling.
fn scale_amplitude(sample: f32, max_range: f32) -> f32 {
    let mag = sample.abs();
    if mag < 1e-4 {
        0.0
    } else {
        // 3.5x gain boost with sqrt curve to amplify low-to-medium audio signals
        let boosted = (mag * 3.5).min(1.0).sqrt();
        sample.signum() * boosted * max_range
    }
}

/// Shared UI data structure protected by Mutex.
pub struct UiData {
    pub track_names: Vec<String>,
    pub track_l_samples: Vec<VecDeque<f32>>,
    pub track_r_samples: Vec<VecDeque<f32>>,
    pub track_osc_msgs: Vec<VecDeque<String>>,
    pub track_last_osc_time: Vec<Option<Instant>>,
    pub master_l_samples: VecDeque<f32>,
    pub master_r_samples: VecDeque<f32>,
    pub master_osc_msgs: VecDeque<String>,
    pub master_last_osc_time: Option<Instant>,
    pub dirty: bool,
}

impl UiData {
    pub fn new(num_tracks: usize) -> Self {
        let mut track_names = Vec::with_capacity(num_tracks);
        let mut track_l_samples = Vec::with_capacity(num_tracks);
        let mut track_r_samples = Vec::with_capacity(num_tracks);
        let mut track_osc_msgs = Vec::with_capacity(num_tracks);
        let mut track_last_osc_time = Vec::with_capacity(num_tracks);

        for i in 0..num_tracks {
            let name = match i {
                0 => "Track 0: TwoOp".to_string(),
                1 => "Track 1: Sampler".to_string(),
                _ => format!("Track {}", i),
            };
            track_names.push(name);
            track_l_samples.push(VecDeque::from(vec![0.0; TRACK_WAVEFORM_ROWS]));
            track_r_samples.push(VecDeque::from(vec![0.0; TRACK_WAVEFORM_ROWS]));
            track_osc_msgs.push(VecDeque::new());
            track_last_osc_time.push(None);
        }

        Self {
            track_names,
            track_l_samples,
            track_r_samples,
            track_osc_msgs,
            track_last_osc_time,
            master_l_samples: VecDeque::from(vec![0.0; MASTER_WAVEFORM_COLS]),
            master_r_samples: VecDeque::from(vec![0.0; MASTER_WAVEFORM_COLS]),
            master_osc_msgs: VecDeque::new(),
            master_last_osc_time: None,
            dirty: true, // Render initial frame
        }
    }

    pub fn push_samples(&mut self, track_outputs: &[(f32, f32)], master_l: f32, master_r: f32) {
        let mut changed = false;

        for (idx, &(l, r)) in track_outputs.iter().enumerate() {
            if idx < self.track_l_samples.len() {
                if self.track_l_samples[idx].len() >= TRACK_WAVEFORM_ROWS {
                    self.track_l_samples[idx].pop_front();
                }
                self.track_l_samples[idx].push_back(l);

                if self.track_r_samples[idx].len() >= TRACK_WAVEFORM_ROWS {
                    self.track_r_samples[idx].pop_front();
                }
                self.track_r_samples[idx].push_back(r);

                if l.abs() > 1e-4 || r.abs() > 1e-4 {
                    changed = true;
                }
            }
        }

        if self.master_l_samples.len() >= MASTER_WAVEFORM_COLS {
            self.master_l_samples.pop_front();
        }
        self.master_l_samples.push_back(master_l);

        if self.master_r_samples.len() >= MASTER_WAVEFORM_COLS {
            self.master_r_samples.pop_front();
        }
        self.master_r_samples.push_back(master_r);

        if master_l.abs() > 1e-4 || master_r.abs() > 1e-4 {
            changed = true;
        }

        if changed {
            self.dirty = true;
        }
    }

    pub fn push_track_osc_msg(&mut self, track_id: usize, msg: String) {
        if track_id < self.track_osc_msgs.len() {
            for line in msg.lines() {
                if self.track_osc_msgs[track_id].len() >= 12 {
                    self.track_osc_msgs[track_id].pop_front();
                }
                self.track_osc_msgs[track_id].push_back(line.to_string());
            }
            self.track_last_osc_time[track_id] = Some(Instant::now());
            self.dirty = true;
        }
    }

    pub fn clear_track_osc_msg(&mut self, track_id: usize) {
        if track_id < self.track_osc_msgs.len() {
            if !self.track_osc_msgs[track_id].is_empty() {
                self.track_osc_msgs[track_id].clear();
                self.track_last_osc_time[track_id] = None;
                self.dirty = true;
            }
        }
    }

    pub fn push_master_osc_msg(&mut self, msg: String) {
        for line in msg.lines() {
            if self.master_osc_msgs.len() >= 6 {
                self.master_osc_msgs.pop_front();
            }
            self.master_osc_msgs.push_back(line.to_string());
        }
        self.master_last_osc_time = Some(Instant::now());
        self.dirty = true;
    }

    pub fn check_inactivity_timeouts(&mut self) {
        let now = Instant::now();

        for t in 0..self.track_osc_msgs.len() {
            if let Some(last_t) = self.track_last_osc_time[t] {
                if now.duration_since(last_t) >= INACTIVITY_TIMEOUT {
                    if !self.track_osc_msgs[t].is_empty() {
                        self.track_osc_msgs[t].clear();
                        self.track_last_osc_time[t] = None;
                        self.dirty = true;
                    }
                }
            }
        }

        if let Some(last_m) = self.master_last_osc_time {
            if now.duration_since(last_m) >= INACTIVITY_TIMEOUT {
                if !self.master_osc_msgs.is_empty() {
                    self.master_osc_msgs.clear();
                    self.master_last_osc_time = None;
                    self.dirty = true;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct UiHandle {
    pub data: Arc<Mutex<UiData>>,
    sample_counter: Arc<Mutex<usize>>,
    peak_track_outputs: Arc<Mutex<Vec<(f32, f32)>>>,
    peak_master: Arc<Mutex<(f32, f32)>>,
}

impl UiHandle {
    pub fn new(num_tracks: usize) -> Self {
        Self {
            data: Arc::new(Mutex::new(UiData::new(num_tracks))),
            sample_counter: Arc::new(Mutex::new(0)),
            peak_track_outputs: Arc::new(Mutex::new(vec![(0.0, 0.0); num_tracks])),
            peak_master: Arc::new(Mutex::new((0.0, 0.0))),
        }
    }

    pub fn record_audio_frame(&self, track_outputs: &[(f32, f32)], master_l: f32, master_r: f32) {
        // Accumulate peak values over 16-sample window so fast transients are captured
        if let Ok(mut peaks) = self.peak_track_outputs.lock() {
            for (idx, &(l, r)) in track_outputs.iter().enumerate() {
                if idx < peaks.len() {
                    if l.abs() > peaks[idx].0.abs() { peaks[idx].0 = l; }
                    if r.abs() > peaks[idx].1.abs() { peaks[idx].1 = r; }
                }
            }
        }
        if let Ok(mut m_peak) = self.peak_master.lock() {
            if master_l.abs() > m_peak.0.abs() { m_peak.0 = master_l; }
            if master_r.abs() > m_peak.1.abs() { m_peak.1 = master_r; }
        }

        let mut count = self.sample_counter.lock().unwrap();
        *count += 1;
        if *count % 16 == 0 {
            let track_peaks = if let Ok(mut peaks) = self.peak_track_outputs.lock() {
                let current = peaks.clone();
                for p in peaks.iter_mut() { *p = (0.0, 0.0); }
                current
            } else {
                vec![(0.0, 0.0); track_outputs.len()]
            };

            let (ml, mr) = if let Ok(mut m_peak) = self.peak_master.lock() {
                let current = *m_peak;
                *m_peak = (0.0, 0.0);
                current
            } else {
                (0.0, 0.0)
            };

            if let Ok(mut data) = self.data.lock() {
                data.push_samples(&track_peaks, ml, mr);
            }
        }
    }

    pub fn push_track_osc(&self, track_id: usize, msg: String) {
        if let Ok(mut data) = self.data.lock() {
            data.push_track_osc_msg(track_id, msg);
        }
    }

    pub fn clear_track_osc(&self, track_id: usize) {
        if let Ok(mut data) = self.data.lock() {
            data.clear_track_osc_msg(track_id);
        }
    }

    pub fn push_master_osc(&self, msg: String) {
        if let Ok(mut data) = self.data.lock() {
            data.push_master_osc_msg(msg);
        }
    }
}

/// Spawns the UI rendering loop thread at ~30 FPS (renders ONLY when dirty).
pub fn start_ui_thread(ui_handle: UiHandle, num_tracks: usize) {
    std::thread::spawn(move || {
        let mut stdout = stdout();
        let _ = execute!(stdout, terminal::Clear(ClearType::All), cursor::Hide);

        let update_interval = Duration::from_millis(33); // ~30 FPS

        loop {
            std::thread::sleep(update_interval);
            let data_arc = ui_handle.data.clone();
            let mut data_guard = match data_arc.lock() {
                Ok(g) => g,
                Err(_) => break,
            };

            // Check 15-second inactivity timeout for clearing messages
            data_guard.check_inactivity_timeouts();

            // Only redraw UI if state is dirty (something changed)
            if !data_guard.dirty {
                continue;
            }
            data_guard.dirty = false;

            // Prepare 80 cols x 43 rows grid buffer
            let mut grid = vec![vec![' '; UI_WIDTH]; UI_HEIGHT];

            // 1. TRACKS SECTION (Rows 0..30)
            // Row 0: Top border
            for c in 0..UI_WIDTH {
                grid[0][c] = '─';
                grid[TRACK_ROWS][c] = '─';
            }
            grid[0][0] = '┌';
            grid[0][UI_WIDTH - 1] = '┐';
            grid[TRACK_ROWS][0] = '├';
            grid[TRACK_ROWS][UI_WIDTH - 1] = '┤';

            // Outer vertical walls for rows 0..30
            for r in 1..TRACK_ROWS {
                grid[r][0] = '│';
                grid[r][UI_WIDTH - 1] = '│';
            }

            // Partition top section for N tracks
            let track_width = if num_tracks > 0 { (UI_WIDTH - 2) / num_tracks } else { UI_WIDTH - 2 };
            for t in 0..num_tracks {
                let left_col = 1 + t * track_width;
                let right_col = if t == num_tracks - 1 { UI_WIDTH - 2 } else { left_col + track_width - 1 };
                let width = right_col - left_col + 1;

                // Draw track column separator if not last track
                if t < num_tracks - 1 {
                    for r in 1..TRACK_ROWS {
                        grid[r][right_col] = '│';
                    }
                    grid[0][right_col] = '┬';
                    grid[TRACK_ROWS][right_col] = '┴';
                }

                // Header at Row 1
                let default_name = format!("Track {}", t);
                let name_str = data_guard.track_names.get(t).unwrap_or(&default_name);
                let header = if name_str.len() > width { &name_str[..width] } else { name_str };
                let start_c = left_col + (width.saturating_sub(header.len())) / 2;
                for (idx, ch) in header.chars().enumerate() {
                    if start_c + idx < right_col {
                        grid[1][start_c + idx] = ch;
                    }
                }

                // Dual vertical waveforms for L and R output channels (Rows 2..29)
                let chan_w = width / 2;
                let center_l = left_col + chan_w / 2;
                let center_r = left_col + chan_w + chan_w / 2;
                let max_h_disp = (chan_w as f32 * 0.45).max(1.0);

                // Flat vertical lines for silence
                for r in 2..TRACK_ROWS {
                    if center_l < right_col { grid[r][center_l] = '│'; }
                    if center_r < right_col { grid[r][center_r] = '│'; }
                }

                if let (Some(l_samples), Some(r_samples)) = (data_guard.track_l_samples.get(t), data_guard.track_r_samples.get(t)) {
                    let num_s = l_samples.len().min(TRACK_WAVEFORM_ROWS);
                    for r_idx in 0..num_s {
                        let r = 2 + r_idx;
                        let s_l = l_samples[r_idx];
                        let s_r = r_samples[r_idx];

                        let disp_l = scale_amplitude(s_l, max_h_disp).round() as i32;
                        let disp_r = scale_amplitude(s_r, max_h_disp).round() as i32;

                        let target_l = (center_l as i32 + disp_l).clamp(left_col as i32, (left_col + chan_w - 1) as i32) as usize;
                        let target_r = (center_r as i32 + disp_r).clamp((left_col + chan_w) as i32, (right_col - 1) as i32) as usize;

                        grid[r][target_l] = '│';
                        grid[r][target_r] = '│';
                    }
                }

                // Recent OSC message lines printed on separate newlines in middle area (Rows 10..22)
                if let Some(msg_queue) = data_guard.track_osc_msgs.get(t) {
                    let start_r = 10;
                    for (i, line) in msg_queue.iter().enumerate().take(12) {
                        let r = start_r + i;
                        let display_text = if line.len() > width { &line[..width] } else { line };
                        let start_col = left_col + (width.saturating_sub(display_text.len())) / 2;
                        for (j, ch) in display_text.chars().enumerate() {
                            if start_col + j < right_col {
                                grid[r][start_col + j] = ch;
                            }
                        }
                    }
                }
            }

            // 2. MASTER & VECTORSCOPE SECTION (Rows 31 to 42)
            // Row 31: Top border for Master & Vectorscope (Divider at Col 60)
            for c in 0..UI_WIDTH {
                grid[MASTER_START_ROW][c] = '─';
                grid[UI_HEIGHT - 1][c] = '─';
            }
            grid[MASTER_START_ROW][0] = '├';
            grid[MASTER_START_ROW][UI_WIDTH - 1] = '┤';
            grid[MASTER_START_ROW][MASTER_COLS] = '┬';

            grid[UI_HEIGHT - 1][0] = '└';
            grid[UI_HEIGHT - 1][UI_WIDTH - 1] = '┘';
            grid[UI_HEIGHT - 1][MASTER_COLS] = '┴';

            // Outer vertical walls & divider at Col 60 for rows 32..41
            for r in (MASTER_START_ROW + 1)..(UI_HEIGHT - 1) {
                grid[r][0] = '│';
                grid[r][MASTER_COLS] = '│';
                grid[r][UI_WIDTH - 1] = '│';
            }

            // Master Header at Row 32 (Col 0..60)
            let m_hdr = "MASTER OUTPUT (L+R)";
            for (j, ch) in m_hdr.chars().enumerate() {
                if 2 + j < MASTER_COLS {
                    grid[MASTER_START_ROW + 1][2 + j] = ch;
                }
            }

            // Single Master horizontal waveform combining (L + R) / 2 at Row 37
            let master_center_row = 37;
            for c in 1..MASTER_COLS {
                grid[master_center_row][c] = '─';
            }

            let num_m = data_guard.master_l_samples.len().min(MASTER_WAVEFORM_COLS);
            for c_idx in 0..num_m {
                let col = 1 + c_idx;
                let s_l = data_guard.master_l_samples[c_idx];
                let s_r = data_guard.master_r_samples[c_idx];
                let combined = (s_l + s_r) * 0.5;

                // Scale vertical displacement to fill ±4 rows
                let disp = scale_amplitude(combined, 4.0).round() as i32;
                let target_row = (master_center_row as i32 - disp).clamp(33, 41) as usize;

                // Fill vertical ribbon between center and target row for prominent visual display
                if disp > 0 {
                    for r in (target_row..=master_center_row).rev() {
                        grid[r][col] = if r == target_row { '▲' } else { '█' };
                    }
                } else if disp < 0 {
                    for r in master_center_row..=target_row {
                        grid[r][col] = if r == target_row { '▼' } else { '█' };
                    }
                } else {
                    grid[master_center_row][col] = '─';
                }
            }

            // Master recent OSC messages printed line-by-line on newlines in rows 33..36
            for (i, line) in data_guard.master_osc_msgs.iter().enumerate().take(4) {
                let r = 33 + i;
                let display_text = if line.len() > MASTER_COLS - 2 { &line[..MASTER_COLS - 2] } else { line };
                for (j, ch) in display_text.chars().enumerate() {
                    grid[r][2 + j] = ch;
                }
            }

            // 3. Bottom-Right XY Vectorscope (Cols 60..80, Rows 31..42)
            // Header at Row 32 inside Scope box
            let v_hdr = "L x R Scope";
            let v_start = MASTER_COLS + 1 + (20 - 1 - v_hdr.len()) / 2;
            for (j, ch) in v_hdr.chars().enumerate() {
                if v_start + j < UI_WIDTH - 1 {
                    grid[MASTER_START_ROW + 1][v_start + j] = ch;
                }
            }

            // Vectorscope Center Axes at Col 70, Row 37
            let vec_center_col = 70;
            let vec_center_row = 37;
            for r in 33..42 {
                grid[r][vec_center_col] = '│';
            }
            for c in (MASTER_COLS + 1)..(UI_WIDTH - 1) {
                grid[vec_center_row][c] = '─';
            }
            grid[vec_center_row][vec_center_col] = '┼';

            // Plot stereo phase correlation points with scaled amplitude
            let s_len = data_guard.master_l_samples.len();
            for i in 0..s_len {
                let l = data_guard.master_l_samples[i];
                let r = data_guard.master_r_samples[i];

                let scaled_l = scale_amplitude(l, 8.5);
                let scaled_r = scale_amplitude(r, 3.5);

                // X maps Left [-1.0, 1.0] -> cols [61, 78] (center 70)
                let x_col = (vec_center_col as f32 + scaled_l).round().clamp(61.0, 78.0) as usize;
                // Y maps Right [-1.0, 1.0] -> rows [41, 33] (center 37)
                let y_row = (vec_center_row as f32 - scaled_r).round().clamp(33.0, 41.0) as usize;

                grid[y_row][x_col] = '*';
            }

            // Render 80 cols x 43 rows without trailing newline on line 42 (prevents terminal scroll)
            let mut output_str = String::with_capacity(UI_WIDTH * UI_HEIGHT + 100);
            for r in 0..UI_HEIGHT {
                let line: String = grid[r].iter().collect();
                output_str.push_str(&line);
                if r < UI_HEIGHT - 1 {
                    output_str.push('\n');
                }
            }

            let _ = execute!(stdout, cursor::MoveTo(0, 0), Print(output_str), cursor::MoveTo(0, UI_HEIGHT as u16));
            let _ = stdout.flush();
        }
    });
}
