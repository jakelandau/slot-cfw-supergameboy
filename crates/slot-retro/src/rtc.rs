const MGBA: usize = 48;
/// Gambatte is no longer one of slot's cores, but cards in the wild still carry the eight byte
/// clock tail it wrote, so its format stays here to read those saves into SameBoy.
const GAMBATTE: usize = 8;
/// SameBoy's `GB_SECTION(rtc)`: rtc_real and rtc_latched as 5 byte clock registers, a uint64
/// `last_rtc_second` at 16, `rtc_cycles` at 24 and `tpp1_mr4` at 28, padded to 32 by the union's
/// eight byte alignment. SameBoy syncs the clock by advancing rtc_real from `last_rtc_second`
/// towards `time(NULL)`, so as on the other two cores a cart's clock is a zero point subtracted
/// from the wall clock: `last_rtc_second - rtc_real`.
const SAMEBOY: usize = 32;
const SAMEBOY_LAST: usize = 16;

const WRAP: u64 = 0x1FF * 86_400;

pub fn convert(tail: &[u8], want: usize, now: u64) -> Option<Vec<u8>> {
    match (tail.len(), want) {
        (MGBA, GAMBATTE) => Some(mgba_to_gambatte(tail).to_le_bytes().to_vec()),
        (GAMBATTE, MGBA) => Some(gambatte_to_mgba(
            u64::from_le_bytes(tail.try_into().ok()?),
            now,
        )),
        (MGBA, SAMEBOY) => Some(sameboy_tail(mgba_to_gambatte(tail), now)),
        (GAMBATTE, SAMEBOY) => Some(sameboy_tail(u64::from_le_bytes(tail.try_into().ok()?), now)),
        (SAMEBOY, GAMBATTE) => Some(sameboy_base(tail).to_le_bytes().to_vec()),
        (SAMEBOY, MGBA) => Some(gambatte_to_mgba(sameboy_base(tail), now)),
        _ => None,
    }
}

/// The five byte hardware clock register a Game Boy cart time is stored in: seconds, minutes,
/// hours, days, then bit 0 of the day count. Halting and overflow bits are left clear so the
/// clock keeps running on the core that receives it.
fn time_bytes(t: u64) -> [u8; 5] {
    let days = t / 86_400;
    [
        (t % 60) as u8,
        (t / 60 % 60) as u8,
        (t / 3_600 % 24) as u8,
        (days & 0xFF) as u8,
        ((days >> 8) & 1) as u8,
    ]
}

fn time_total(bytes: &[u8]) -> u64 {
    u64::from(bytes[0])
        + 60 * u64::from(bytes[1])
        + 3_600 * u64::from(bytes[2])
        + 86_400 * (u64::from(bytes[3]) + ((u64::from(bytes[4]) & 1) << 8))
}

fn sameboy_base(tail: &[u8]) -> u64 {
    let last = u64::from_le_bytes(tail[SAMEBOY_LAST..SAMEBOY_LAST + 8].try_into().unwrap());
    last.wrapping_sub(time_total(&tail[..5]))
}

fn sameboy_tail(base: u64, now: u64) -> Vec<u8> {
    let mut t = now.wrapping_sub(base);
    if t > WRAP {
        t = (t - 1) % WRAP + 1;
    }
    let mut out = vec![0u8; SAMEBOY];
    let clock = time_bytes(t);
    out[..5].copy_from_slice(&clock);
    out[5..10].copy_from_slice(&clock);
    out[SAMEBOY_LAST..SAMEBOY_LAST + 8].copy_from_slice(&now.to_le_bytes());
    out
}

fn mgba_to_gambatte(tail: &[u8]) -> u64 {
    let reg = |i: usize| u64::from(tail[20 + i * 4]);
    let days = reg(3) + ((reg(4) & 1) << 8);
    let latched = reg(0) + 60 * reg(1) + 3_600 * reg(2) + 86_400 * days;
    let unix = u64::from_le_bytes(tail[40..48].try_into().unwrap());
    unix.wrapping_sub(latched)
}

fn gambatte_to_mgba(base: u64, now: u64) -> Vec<u8> {
    let mut t = now.wrapping_sub(base);
    if t > WRAP {
        t = (t - 1) % WRAP + 1;
    }
    let days = t / 86_400;
    let regs = [
        (t % 60) as u32,
        (t / 60 % 60) as u32,
        (t / 3_600 % 24) as u32,
        (days & 0xFF) as u32,
        (days >> 8) as u32,
    ];
    let mut out = Vec::with_capacity(MGBA);
    for r in regs.iter().chain(regs.iter()) {
        out.extend_from_slice(&r.to_le_bytes());
    }
    out.extend_from_slice(&now.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgba_tail(current: [u32; 5], latched: [u32; 5], unix: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(MGBA);
        for r in current.iter().chain(latched.iter()) {
            out.extend_from_slice(&r.to_le_bytes());
        }
        out.extend_from_slice(&unix.to_le_bytes());
        out
    }

    fn registers(tail: &[u8], at: usize) -> [u32; 5] {
        std::array::from_fn(|i| {
            u32::from_le_bytes(tail[at + i * 4..at + i * 4 + 4].try_into().unwrap())
        })
    }

    fn unix(tail: &[u8]) -> u64 {
        u64::from_le_bytes(tail[40..48].try_into().unwrap())
    }

    #[test]
    fn mgba_to_gambatte_subtracts_the_latched_time() {
        let tail = mgba_tail([0; 5], [5, 4, 3, 300 & 0xFF, 1], 1_800_000_000);
        let out = convert(&tail, GAMBATTE, 1_900_000_000).expect("no conversion");
        assert_eq!(
            u64::from_le_bytes(out[..].try_into().unwrap()),
            1_800_000_000 - (5 + 240 + 10_800 + 300 * 86_400)
        );
    }

    #[test]
    fn gambatte_to_mgba_splits_elapsed_time() {
        let now = 1_800_000_000u64;
        let base = (now - 90_061).to_le_bytes();
        let out = convert(&base, MGBA, now).expect("no conversion");
        assert_eq!(out.len(), MGBA);
        assert_eq!(registers(&out, 0), [1, 1, 1, 1, 0]);
        assert_eq!(registers(&out, 20), [1, 1, 1, 1, 0]);
        assert_eq!(unix(&out), now);
    }

    #[test]
    fn gambatte_to_mgba_wraps_past_511_days_like_gambatte() {
        let now = 1_800_000_000u64;
        let at = |days: u64, extra: u64| {
            let base = (now - days * 86_400 - extra).to_le_bytes();
            convert(&base, MGBA, now).expect("no conversion")
        };
        assert_eq!(registers(&at(600, 0), 0), [0, 0, 0, 89, 0]);
        assert_eq!(registers(&at(600, 0), 20), [0, 0, 0, 89, 0]);
        assert_eq!(registers(&at(511, 0), 0), [0, 0, 0, 255, 1]);
        assert_eq!(registers(&at(512, 0), 0), [0, 0, 0, 1, 0]);
        assert_eq!(registers(&at(600, 3_661), 0), [1, 1, 1, 89, 0]);
        assert_eq!(unix(&at(600, 0)), now);
    }

    #[test]
    fn mgba_round_trips_through_gambatte() {
        let now = 1_800_000_000u64;
        let tail = mgba_tail([9; 5], [5, 4, 3, 5, 0], now - 100);
        let gambatte = convert(&tail, GAMBATTE, now).expect("no conversion");
        let back = convert(&gambatte, MGBA, now).expect("no conversion back");
        assert_eq!(registers(&back, 20), [45, 5, 3, 5, 0]);
        assert_eq!(registers(&back, 0), [45, 5, 3, 5, 0]);
        assert_eq!(unix(&back), now);
    }

    fn sameboy_tail(base: u64, now: u64) -> Vec<u8> {
        convert(&base.to_le_bytes(), SAMEBOY, now).expect("no conversion")
    }

    #[test]
    fn sameboy_keeps_its_clock_where_its_rtc_section_expects_it() {
        let now = 1_800_000_000u64;
        let out = sameboy_tail(now - 90_061, now);
        assert_eq!(out.len(), SAMEBOY);
        assert_eq!(&out[..5], &[1, 1, 1, 1, 0], "rtc_real");
        assert_eq!(&out[5..10], &[1, 1, 1, 1, 0], "rtc_latched");
        assert_eq!(u64::from_le_bytes(out[16..24].try_into().unwrap()), now);
        assert_eq!(&out[24..], &[0; 8], "rtc_cycles and padding");
    }

    #[test]
    fn sameboy_to_gambatte_reads_the_clock_it_syncs_to() {
        let now = 1_800_000_000u64;
        let base = now - 500_000;
        let back = convert(&sameboy_tail(base, now), GAMBATTE, now + 10).expect("no conversion");
        assert_eq!(u64::from_le_bytes(back[..].try_into().unwrap()), base);
    }

    #[test]
    fn sameboy_latches_the_days_bit_into_the_high_byte() {
        let now = 1_800_000_000u64;
        let out = sameboy_tail(now - 400 * 86_400 - 3_661, now);
        assert_eq!(&out[..5], &[1, 1, 1, 144, 1]);
    }

    #[test]
    fn mgba_round_trips_through_sameboy() {
        let now = 1_800_000_000u64;
        let tail = mgba_tail([9; 5], [5, 4, 3, 5, 0], now - 100);
        let sameboy = convert(&tail, SAMEBOY, now).expect("no conversion");
        let back = convert(&sameboy, MGBA, now).expect("no conversion back");
        assert_eq!(registers(&back, 20), [45, 5, 3, 5, 0]);
        assert_eq!(unix(&back), now);
    }

    #[test]
    fn sameboy_wraps_past_511_days_like_gambatte() {
        let now = 1_800_000_000u64;
        let out = sameboy_tail(now - 512 * 86_400, now);
        assert_eq!(&out[..5], &[0, 0, 0, 1, 0]);
    }

    #[test]
    fn other_sizes_are_left_alone() {
        assert_eq!(convert(&[0; MGBA], MGBA, 0), None);
        assert_eq!(convert(&[0; GAMBATTE], GAMBATTE, 0), None);
        assert_eq!(convert(&[0; SAMEBOY], SAMEBOY, 0), None);
        assert_eq!(convert(&[0; 12], GAMBATTE, 0), None);
        assert_eq!(convert(&[0; MGBA], 16, 0), None);
        assert_eq!(convert(&[0; GAMBATTE], 16, 0), None);
        assert_eq!(convert(&[0; SAMEBOY], 16, 0), None);
        assert_eq!(convert(&[], MGBA, 0), None);
    }
}
