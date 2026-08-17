const INITIAL_STATE: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0100_0000_01b3;

pub const fn fnv1a(bytes: &[u8]) -> u64 {
  let mut hash = INITIAL_STATE;
  let mut i = 0;
  while i < bytes.len() {
    hash ^= bytes[i] as u64;
    hash = hash.wrapping_mul(PRIME);
    i += 1;
  }
  return hash;
}

pub fn file_hash(content: &str) -> u64 {
  return fnv1a(content.as_bytes());
}
