use knuffel::Decode;

#[derive(Decode, Debug, Clone, PartialEq, Eq)]
pub struct Hold {
    #[knuffel(property)]
    pub fingers: i32,
    #[knuffel(property)]
    pub action: Option<String>,
}
