#[derive(Clone, Debug)]
pub struct IdentifierMangler {
    state: u32,
}

impl IdentifierMangler {
    pub fn new(seed: u32) -> Self {
        Self { state: seed.max(1) }
    }

    pub fn next_ident(&mut self) -> String {
        self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
        let mut value = self.state as usize;
        let alphabet = b"abcdefghijklmnopqrstuvwxyz";
        let mut out = String::from("_");
        loop {
            out.push(alphabet[value % alphabet.len()] as char);
            value /= alphabet.len();
            if value == 0 {
                break;
            }
        }
        out
    }
}
