//! Reusable building blocks shared by all four APU channels.

/// Volume envelope direction encoded in NRx2 bit 3.
///
/// The envelope steps the DAC volume up (`Increase`) or down (`Decrease`)
/// by 1 each time the envelope timer expires (once per 64 Hz tick from
/// frame-sequencer step 7).  A period of 0 freezes the envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnvDir {
    #[default]
    Decrease = 0,
    Increase = 1,
}

impl EnvDir {
    pub fn from_bit(b: bool) -> Self {
        if b { Self::Increase } else { Self::Decrease }
    }
}

/// Length counter shared by all four channels.
///
/// When `enabled`, the counter decrements once per length clock (256 Hz;
/// frame-sequencer steps 0, 2, 4, 6).  When it reaches zero the channel
/// is silenced.  The counter is loaded with `64 − NRx1[5:0]` for Ch1/2/4,
/// or `256 − NR31` for Ch3.
#[derive(Debug, Default)]
pub struct LengthCounter {
    pub value:   u16,  // remaining ticks (u16 covers Ch3's 256-step range)
    pub enabled: bool, // NRx4 bit 6: stop channel when counter expires
}

/// Volume envelope used by Ch1, Ch2, and Ch4.
///
/// On trigger, `volume` is loaded from `initial` and `timer` from `period`.
/// Every envelope clock the timer decrements; when it hits 0 `volume` steps
/// by ±1 and the timer reloads from `period`.
#[derive(Debug, Default)]
pub struct VolumeEnvelope {
    pub volume:  u8,     // current DAC volume (0–15)
    pub initial: u8,     // NRx2 bits 4–7: volume loaded on trigger
    pub dir:     EnvDir, // NRx2 bit 3: Increase or Decrease
    pub period:  u8,     // NRx2 bits 0–2: envelope period; 0 = frozen
    pub timer:   u8,     // internal countdown; reloads from period
}
