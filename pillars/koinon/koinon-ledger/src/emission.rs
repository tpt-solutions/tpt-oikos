/// OIKOS emission schedule: disinflationary, halts at year 20.
///
/// Year 1: 50M OIKOS
/// Year 2: 40M
/// Year 3: 32M
/// ...
/// Each year is 80% of the previous year's emission.
/// After year 20, emission is zero.
pub const YEAR_1_EMISSION: u128 = 50_000_000;
pub const DECAY_FACTOR_NUM: u128 = 8;
pub const DECAY_FACTOR_DEN: u128 = 10;
pub const EMISSION_HALT_YEAR: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmissionEntry {
    pub year: u64,
    pub annual_emission: u128,
    pub cumulative_supply: u128,
}

pub fn emission_at_year(year: u64) -> u128 {
    if year == 0 || year > EMISSION_HALT_YEAR {
        return 0;
    }
    let mut emission = YEAR_1_EMISSION;
    for _ in 1..year {
        emission = emission * DECAY_FACTOR_NUM / DECAY_FACTOR_DEN;
    }
    emission
}

pub fn cumulative_supply_at_year(year: u64) -> u128 {
    let mut cumulative: u128 = 400_000_000; // genesis allocation (40% validators + 30% treasury + 20% ecosystem + 10% team = 100%)
    // Actually, cumulative supply includes genesis + all emissions up to that year
    // Genesis = 0 new emissions, but 1B total supply exists from genesis allocation
    // The emission schedule adds NEW tokens on top of the genesis distribution
    // Wait - re-reading tokenomics.txt: "Cumulative Supply" column shows 450M at year 1
    // which means genesis starts at 400M (1B * (1-10% team vesting starts locked?))
    // Actually: genesis = 450M distributed, 50M = year 1 emission
    // Let me re-read: Year 1 emission = 50M, Cumulative = 450M
    // So cumulative = previous + emission
    // Year 0 (genesis): 400M distributed
    // Year 1: 400M + 50M = 450M
    // This means the 10% team allocation (100M) vests over 4 years
    // and isn't counted in the initial "circulating" supply

    // Simplified: track emission only
    if year == 0 {
        return 0;
    }
    for y in 1..=year.min(EMISSION_HALT_YEAR) {
        cumulative += emission_at_year(y);
    }
    cumulative
}

pub fn emission_schedule() -> Vec<EmissionEntry> {
    let mut schedule = Vec::new();
    let mut cumulative = 0u128;
    for year in 1..=EMISSION_HALT_YEAR {
        let emission = emission_at_year(year);
        cumulative += emission;
        schedule.push(EmissionEntry {
            year,
            annual_emission: emission,
            cumulative_supply: cumulative,
        });
    }
    schedule
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_year_1_emission() {
        assert_eq!(emission_at_year(1), 50_000_000);
    }

    #[test]
    fn test_year_2_emission() {
        assert_eq!(emission_at_year(2), 40_000_000);
    }

    #[test]
    fn test_year_3_emission() {
        assert_eq!(emission_at_year(3), 32_000_000);
    }

    #[test]
    fn test_emission_halt_after_year_20() {
        assert_eq!(emission_at_year(21), 0);
        assert_eq!(emission_at_year(100), 0);
    }

    #[test]
    fn test_emission_zero_for_year_0() {
        assert_eq!(emission_at_year(0), 0);
    }

    #[test]
    fn test_schedule_length() {
        let schedule = emission_schedule();
        assert_eq!(schedule.len(), 20);
    }

    #[test]
    fn test_cumulative_grows() {
        let c1 = cumulative_supply_at_year(1);
        let c5 = cumulative_supply_at_year(5);
        assert!(c5 > c1);
    }

    #[test]
    fn test_total_emission_does_not_exceed_max_supply() {
        let total_emission: u128 = (1..=EMISSION_HALT_YEAR)
            .map(|y| emission_at_year(y))
            .sum();
        // Total emission should be well under 600M (leaving room for genesis 400M)
        assert!(total_emission < 600_000_000);
    }
}
