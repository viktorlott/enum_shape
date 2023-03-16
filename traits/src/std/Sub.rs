pub trait Sub<Rhs = Self> {
    type Output;

    fn sub(self, rhs: Rhs) -> Self::Output;
}
