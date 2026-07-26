use std::time::{Duration, Instant};

/// Preset tick rate — 3 Mode resmi Kombox: Potato (20 TPS), Rice (50 TPS), Beef (100 TPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickRate {
    Potato, // 20 TPS (0.05s / 50ms)
    Rice,   // 50 TPS (0.02s / 20ms)
    Beef,   // 100 TPS (0.01s / 10ms)
}

impl TickRate {
    pub fn tps(self) -> u32 {
        match self {
            TickRate::Potato => 20,
            TickRate::Rice => 50,
            TickRate::Beef => 100,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TickRate::Potato => "Potato (20 TPS)",
            TickRate::Rice => "Rice (50 TPS)",
            TickRate::Beef => "Beef (100 TPS)",
        }
    }

    /// Durasi satu tick dalam detik (dt)
    pub fn dt_seconds(self) -> f32 {
        1.0 / self.tps() as f32
    }
}

/// Accumulator pattern — tick loop jalan di rate tetap (dt konstan),
/// independen dari frame rate render yang variable.
pub struct TickClock {
    pub mode: TickRate,
    accumulator: f32,
    last_instant: Instant,
    pub tick_count: u64,
}

impl TickClock {
    pub fn new(mode: TickRate) -> Self {
        Self {
            mode,
            accumulator: 0.0,
            last_instant: Instant::now(),
            tick_count: 0,
        }
    }

    pub fn set_mode(&mut self, mode: TickRate) {
        self.mode = mode;
        self.accumulator = 0.0;
    }

    /// Panggil tiap frame render. Balikin berapa kali tick harus dijalanin
    pub fn ticks_to_run(&mut self) -> u32 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_instant).as_secs_f32();
        self.last_instant = now;

        // Clamp biar gak "spiral of death" kalau ada freeze panjang
        let elapsed = elapsed.min(0.25);

        self.accumulator += elapsed;
        let dt = self.mode.dt_seconds();

        let mut count = 0;
        while self.accumulator >= dt {
            self.accumulator -= dt;
            self.tick_count += 1;
            count += 1;
        }
        count
    }

    pub fn dt(&self) -> f32 {
        self.mode.dt_seconds()
    }
}

pub struct Cooldown {
    pub remaining_seconds: f32,
}

impl Cooldown {
    pub fn new(seconds: f32) -> Self {
        Self { remaining_seconds: seconds }
    }

    pub fn tick(&mut self, dt: f32) {
        if self.remaining_seconds > 0.0 {
            self.remaining_seconds -= dt;
        }
    }

    pub fn is_ready(&self) -> bool {
        self.remaining_seconds <= 0.0
    }
}

#[allow(dead_code)]
pub fn placeholder_duration() -> Duration {
    Duration::from_secs(0)
}
