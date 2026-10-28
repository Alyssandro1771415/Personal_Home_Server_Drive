#[derive(Debug)]
pub enum Erro {
    Interno,
    NaoAutorizado,
    Validacao,
    Conflito,
    NaoEncontrado,
}
