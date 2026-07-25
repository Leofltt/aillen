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
const DISPLAY_SLOTS: usize = 4; // Always show exactly 4 track slots in the UI

const TRACK_ROWS: usize = 15; // Rows 0..15 and 16..30
const MASTER_START_ROW: usize = 31; // Rows 31..42

const MASTER_COLS: usize = 60; // 60 cols for master, 20 cols for scope
const TRACK_WAVEFORM_ROWS: usize = 11; // Rows 4..15 and 19..30 for track vertical waveforms
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
    /// Per-track last activity timestamp (audio or OSC) for LRU display selection.
    pub track_last_activity: Vec<Option<Instant>>,
    /// The track IDs currently occupying the 4 display slots (ordered by slot position).
    pub displayed_tracks: Vec<usize>,
    /// Per-track memory of the last slot occupied (for sticky positioning).
    pub preferred_slot: Vec<Option<usize>>,
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
        let mut track_last_activity = Vec::with_capacity(num_tracks);
        let mut preferred_slot = Vec::with_capacity(num_tracks);

        for i in 0..num_tracks {
            let name = match i {
                0 => "Track 0: TwoOp".to_string(),
                1 => "Track 1: Sampler".to_string(),
                2 => "Track 2: Sampler".to_string(),
                3 => "Track 3: Sampler".to_string(),
                4 => "Track 4: TwoOp".to_string(),
                5 => "Track 5: Sampler".to_string(),
                6 => "Track 6: Synth303".to_string(),
                _ => format!("Track {}", i),
            };
            track_names.push(name);
            track_l_samples.push(VecDeque::from(vec![0.0; TRACK_WAVEFORM_ROWS]));
            track_r_samples.push(VecDeque::from(vec![0.0; TRACK_WAVEFORM_ROWS]));
            track_osc_msgs.push(VecDeque::new());
            track_last_osc_time.push(None);
            track_last_activity.push(None);
            // Seed preferred slots for the initial display tracks
            if i < DISPLAY_SLOTS {
                preferred_slot.push(Some(i));
            } else {
                preferred_slot.push(None);
            }
        }

        // Seed the initial 4 display slots with the first N tracks (up to DISPLAY_SLOTS)
        let initial_displayed: Vec<usize> = (0..DISPLAY_SLOTS.min(num_tracks)).collect();

        Self {
            track_names,
            track_l_samples,
            track_r_samples,
            track_osc_msgs,
            track_last_osc_time,
            track_last_activity,
            displayed_tracks: initial_displayed,
            preferred_slot,
            master_l_samples: VecDeque::from(vec![0.0; MASTER_WAVEFORM_COLS]),
            master_r_samples: VecDeque::from(vec![0.0; MASTER_WAVEFORM_COLS]),
            master_osc_msgs: VecDeque::new(),
            master_last_osc_time: None,
            dirty: true, // Render initial frame
        }
    }

    /// Mark a track as active. If it is not in the current display slots,
    /// evict the least recently active slot and replace it — preferring
    /// to place the track back in its last known slot for visual stability.
    pub fn touch_track(&mut self, track_id: usize) {
        let now = Instant::now();
        if track_id < self.track_last_activity.len() {
            self.track_last_activity[track_id] = Some(now);
        }

        // Already displayed? Nothing to swap.
        if self.displayed_tracks.contains(&track_id) {
            return;
        }

        // Helper: find the LRU (least recently active) slot index
        let find_lru_slot = |displayed: &[usize], activity: &[Option<Instant>]| -> usize {
            let mut oldest_slot = 0;
            let mut oldest_time: Option<Instant> = activity
                .get(displayed[0])
                .copied()
                .flatten();

            for (slot, &tid) in displayed.iter().enumerate().skip(1) {
                let t = activity.get(tid).copied().flatten();
                match (oldest_time, t) {
                    (_, None) => {
                        oldest_slot = slot;
                        break;
                    }
                    (Some(oldest), Some(current)) if current < oldest => {
                        oldest_slot = slot;
                        oldest_time = Some(current);
                    }
                    (None, _) => {}
                    _ => {}
                }
            }
            oldest_slot
        };

        let lru_slot = find_lru_slot(&self.displayed_tracks, &self.track_last_activity);

        // Determine which slot to use: prefer the track's previous slot if
        // its current occupant is not the most recently active displayed track.
        let target_slot = if let Some(Some(pref)) = self.preferred_slot.get(track_id) {
            let pref = *pref;
            if pref < self.displayed_tracks.len() {
                // Check that the preferred slot's occupant is "stale enough" to evict.
                // Find the most recently active slot so we don't evict it.
                let mut newest_slot = 0;
                let mut newest_time: Option<Instant> = None;
                for (slot, &tid) in self.displayed_tracks.iter().enumerate() {
                    let t = self.track_last_activity.get(tid).copied().flatten();
                    match (newest_time, t) {
                        (None, Some(_)) | (_, None) => {
                            if t.is_some() {
                                newest_slot = slot;
                                newest_time = t;
                            }
                        }
                        (Some(best), Some(current)) if current > best => {
                            newest_slot = slot;
                            newest_time = Some(current);
                        }
                        _ => {}
                    }
                }
                // Only reclaim preferred slot if it's not the single most active slot
                if pref != newest_slot {
                    pref
                } else {
                    lru_slot
                }
            } else {
                lru_slot
            }
        } else {
            lru_slot
        };

        // Record the evicted track's slot preference so it can reclaim later
        let evicted_track = self.displayed_tracks[target_slot];
        if evicted_track < self.preferred_slot.len() {
            self.preferred_slot[evicted_track] = Some(target_slot);
        }

        // Place the new track and remember its slot
        self.displayed_tracks[target_slot] = track_id;
        if track_id < self.preferred_slot.len() {
            self.preferred_slot[track_id] = Some(target_slot);
        }
        self.dirty = true;
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
                    self.touch_track(idx);
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
            self.touch_track(track_id);
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
pub fn start_ui_thread(ui_handle: UiHandle, _num_tracks: usize) {
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

            // 1. TRACKS SECTION – 2x2 grid showing the 4 most recently active tracks
            let tracks_per_row: usize = 2;
            let display_count = data_guard.displayed_tracks.len().min(DISPLAY_SLOTS);
            let num_bands = (display_count + tracks_per_row - 1) / tracks_per_row;

            // Horizontal borders
            for c in 0..UI_WIDTH {
                grid[0][c] = '─';                           // top border
                grid[TRACK_ROWS][c] = '─';                  // mid border
                grid[TRACK_ROWS * 2][c] = '─';              // bottom of tracks area
            }
            // Corner / junction chars
            grid[0][0] = '┌';
            grid[0][UI_WIDTH - 1] = '┐';
            grid[TRACK_ROWS][0] = '├';
            grid[TRACK_ROWS][UI_WIDTH - 1] = '┤';
            grid[TRACK_ROWS * 2][0] = '├';
            grid[TRACK_ROWS * 2][UI_WIDTH - 1] = '┤';

            // Outer vertical walls for all track rows (1..TRACK_ROWS*2 - 1)
            for r in 1..(TRACK_ROWS * 2) {
                grid[r][0] = '│';
                grid[r][UI_WIDTH - 1] = '│';
            }

            // Take a snapshot of which tracks to display (to avoid borrow issues)
            let displayed: Vec<usize> = data_guard.displayed_tracks.iter().copied().take(DISPLAY_SLOTS).collect();

            // Render each band of tracks
            let track_width = (UI_WIDTH - 2) / tracks_per_row;
            for band in 0..num_bands {
                let row_offset = band * TRACK_ROWS;
                let band_start = band * tracks_per_row;
                let band_end = (band_start + tracks_per_row).min(display_count);
                let band_count = band_end - band_start;

                for slot_in_band in 0..band_count {
                    let display_slot = band_start + slot_in_band;
                    let t = displayed[display_slot]; // actual track index
                    let left_col = 1 + slot_in_band * track_width;
                    let right_col = if slot_in_band == tracks_per_row - 1 { UI_WIDTH - 2 } else { left_col + track_width - 1 };
                    let width = right_col - left_col + 1;

                    // Draw column separator between tracks in the same band
                    if slot_in_band < band_count - 1 {
                        for r in (row_offset + 1)..(row_offset + TRACK_ROWS) {
                            grid[r][right_col] = '│';
                        }
                        grid[row_offset][right_col] = '┬';
                        grid[row_offset + TRACK_ROWS][right_col] = '┴';
                    }

                    // Header (first content row of this band)
                    let header_row = row_offset + 1;
                    let default_name = format!("Track {}", t);
                    let name_str = data_guard.track_names.get(t).unwrap_or(&default_name);
                    let header = if name_str.len() > width { &name_str[..width] } else { name_str };
                    let start_c = left_col + (width.saturating_sub(header.len())) / 2;
                    for (idx, ch) in header.chars().enumerate() {
                        if start_c + idx < right_col {
                            grid[header_row][start_c + idx] = ch;
                        }
                    }

                    // Dual vertical waveforms (L and R channels)
                    let waveform_start_row = row_offset + 3;
                    let chan_w = width / 2;
                    let center_l = left_col + chan_w / 2;
                    let center_r = left_col + chan_w + chan_w / 2;
                    let max_h_disp = (chan_w as f32 * 0.45).max(1.0);

                    // Flat vertical lines for silence
                    for r in waveform_start_row..(row_offset + TRACK_ROWS) {
                        if center_l < right_col { grid[r][center_l] = '│'; }
                        if center_r < right_col { grid[r][center_r] = '│'; }
                    }

                    if let (Some(l_samples), Some(r_samples)) = (data_guard.track_l_samples.get(t), data_guard.track_r_samples.get(t)) {
                        let num_s = l_samples.len().min(TRACK_WAVEFORM_ROWS);
                        for r_idx in 0..num_s {
                            let r = waveform_start_row + r_idx;
                            if r >= row_offset + TRACK_ROWS { break; }
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

                    // OSC messages in the middle area of this band
                    let osc_start_row = row_offset + 5;
                    let max_osc_lines = (TRACK_ROWS - 6).min(8);
                    if let Some(msg_queue) = data_guard.track_osc_msgs.get(t) {
                        for (i, line) in msg_queue.iter().enumerate().take(max_osc_lines) {
                            let r = osc_start_row + i;
                            if r >= row_offset + TRACK_ROWS { break; }
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
