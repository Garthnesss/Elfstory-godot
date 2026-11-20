pub mod rng;

#[cfg(test)]
mod tests {
    use super::rng::Rng;

    #[test]
    fn seeded_rng_is_deterministic() {
        let mut a = Rng::new(0xCAFE_1234);
        let mut b = Rng::new(0xCAFE_1234);
        for _ in 0..32 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }
}
