//! pt-BR UI strings (D12 — user-facing text in pt-BR only).
//!
//! The UI references these as it is wired up (T1-3); until then, not every
//! constant has a call site, so dead-code is allowed at module level.
#![allow(dead_code)]

pub const APP_TITLE: &str = "Das Auto";
pub const SUBTITLE: &str = "Seu carro, na palma da mão";

// Tabs
pub const TAB_HOME: &str = "Início";
pub const TAB_EXPENSES: &str = "Despesas";
pub const TAB_DOCUMENTS: &str = "Documentos";
pub const TAB_PARTS: &str = "Peças";
pub const TAB_PROFILE: &str = "Perfil";

// Auth
pub const AUTH_SIGNIN_TITLE: &str = "Entrar";
pub const AUTH_SIGNUP_TITLE: &str = "Criar conta";
pub const AUTH_EMAIL: &str = "E-mail";
pub const AUTH_PASSWORD: &str = "Senha";
pub const AUTH_GOOGLE: &str = "Continuar com Google";
pub const AUTH_GOOGLE_LOADING: &str = "Abrindo Google...";
pub const AUTH_OR: &str = "ou";
pub const AUTH_LOADING: &str = "Carregando...";
pub const AUTH_SWITCH_TO_SIGNUP: &str = "Não tem conta? Criar";
pub const AUTH_SWITCH_TO_SIGNIN: &str = "Já tem conta? Entrar";

// Wizard
pub const WIZARD_TITLE_1: &str = "Novo veículo — 1 de 2";
pub const WIZARD_TITLE_2: &str = "Novo veículo — 2 de 2";
pub const WIZARD_MAKE: &str = "Marca";
pub const WIZARD_MODEL: &str = "Modelo";
pub const WIZARD_VERSION: &str = "Versão (opcional)";
pub const WIZARD_YEAR: &str = "Ano";
pub const WIZARD_FUEL: &str = "Combustível";
pub const WIZARD_PLATE: &str = "Placa";
pub const WIZARD_UF: &str = "UF";
pub const WIZARD_RENAVAM: &str = "Renavam";
pub const WIZARD_RENAVAM_HINT: &str = "11 dígitos (opcional)";
pub const WIZARD_NEXT: &str = "Avançar";
pub const WIZARD_BACK: &str = "Voltar";
pub const WIZARD_SAVE: &str = "Adicionar veículo";
pub const WIZARD_SAVING: &str = "Salvando...";
pub const WIZARD_FUEL_EMPTY: &str = "Selecione";

// Fuel labels (value = the entity's serde lowercase name)
pub const FUEL_GASOLINE: &str = "Gasolina";
pub const FUEL_ETHANOL: &str = "Álcool (etanol)";
pub const FUEL_FLEX: &str = "Flex";
pub const FUEL_DIESEL: &str = "Diesel";
pub const FUEL_HYBRID: &str = "Híbrido";
pub const FUEL_ELECTRIC: &str = "Elétrico";

/// (UF sigla, nome) — all 27.
pub const UFS: [(&str, &str); 27] = [
    ("AC", "Acre"),
    ("AL", "Alagoas"),
    ("AM", "Amazonas"),
    ("AP", "Amapá"),
    ("BA", "Bahia"),
    ("CE", "Ceará"),
    ("DF", "Distrito Federal"),
    ("ES", "Espírito Santo"),
    ("GO", "Goiás"),
    ("MA", "Maranhão"),
    ("MT", "Mato Grosso"),
    ("MS", "Mato Grosso do Sul"),
    ("MG", "Minas Gerais"),
    ("PA", "Pará"),
    ("PB", "Paraíba"),
    ("PR", "Paraná"),
    ("PE", "Pernambuco"),
    ("PI", "Piauí"),
    ("RJ", "Rio de Janeiro"),
    ("RN", "Rio Grande do Norte"),
    ("RS", "Rio Grande do Sul"),
    ("RO", "Rondônia"),
    ("RR", "Roraima"),
    ("SC", "Santa Catarina"),
    ("SP", "São Paulo"),
    ("SE", "Sergipe"),
    ("TO", "Tocantins"),
];

// Home
pub const HOME_EMPTY_TITLE: &str = "Adicione seu primeiro veículo";
pub const HOME_EMPTY_TEXT: &str =
    "Para começar, cadastre a marca, o modelo e a placa do seu carro.";
pub const HOME_ADD: &str = "Adicionar veículo";
pub const HOME_VEHICLES: &str = "Seus veículos";
pub const HOME_SYNCING: &str = "Sincronizando...";
pub const HOME_TAB_EMPTY: &str = "Em construção — fase 1/2.";
pub const HOME_LOGOUT: &str = "Sair";
pub const HOME_REMOVE: &str = "Remover";
pub const HOME_REMOVE_CANCEL: &str = "Cancelar";
pub const HOME_REMOVE_Q: &str = "Remover veículo?";

// Expenses (T2-1)
pub const EXP_SELECT_VEHICLE: &str = "Veículo";
pub const EXP_MONTH: &str = "este mês";
pub const EXP_DELTA_UP: &str = "↑ {}% vs. mês anterior";
pub const EXP_DELTA_DOWN: &str = "↓ {}% vs. mês anterior";
pub const EXP_DELTA_SAME: &str = "igual ao mês anterior";
pub const EXP_DELTA_NONE: &str = "sem despesas no mês anterior";
pub const EXP_NEW: &str = "Nova despesa";
pub const EXP_CATEGORY: &str = "Categoria";
pub const EXP_TITLE_FIELD: &str = "Descrição";
pub const EXP_AMOUNT: &str = "Valor (R$)";
pub const EXP_DATE: &str = "Data";
pub const EXP_ODOMETER: &str = "Odômetro (km, opcional)";
pub const EXP_LOCATION: &str = "Local (opcional)";
pub const EXP_CANCEL: &str = "Cancelar";
pub const EXP_SAVE: &str = "Salvar despesa";
pub const EXP_SAVING: &str = "Salvando...";
pub const EXP_CAT_FUEL: &str = "Combustível";
pub const EXP_CAT_MAINT: &str = "Manutenção";
pub const EXP_CAT_TAX: &str = "Imposto";
pub const EXP_CAT_INSURANCE: &str = "Seguro";
pub const EXP_CAT_TIRE: &str = "Pneu";
pub const EXP_CAT_FINE: &str = "Multa";
pub const EXP_CAT_OTHER: &str = "Outro";
pub const EXP_EMPTY_LIST: &str = "Nenhuma despesa registrada para este veículo.";
pub const EXP_REMOVE: &str = "Remover";
pub const EXP_REMOVE_Q: &str = "Remover despesa?";
pub const EXP_ERR_TITLE: &str = "Informe uma descrição.";
pub const EXP_ERR_AMOUNT: &str = "Informe um valor maior que zero.";
pub const EXP_ERR_DATE: &str = "Data inválida.";
pub const EXP_ERR_ODOMETER: &str = "Odômetro deve ser de 0 a 3.000.000 km.";

/// Month names (pt-BR), 1-indexed access by `YearMonth.month()`.
pub const MONTH_NAMES: [&str; 12] = [
    "Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho", "Julho", "Agosto", "Setembro",
    "Outubro", "Novembro", "Dezembro",
];

/// "Novembro de 2026" — month name + year (T2-1 summaries).
pub fn month_label(year: i32, month: u32) -> String {
    format!("{} de {}", MONTH_NAMES[(month as usize).wrapping_sub(1)], year)
}

/// Formats an amount in cents as a pt-BR string: `1234567` -> "R$ 1.234,57".
pub fn money_str(cents: i64) -> String {
    let neg = cents < 0;
    let c = cents.unsigned_abs();
    let int = c / 100;
    let frac = c % 100;
    let chars: Vec<char> = int.to_string().chars().collect();
    let mut out = String::new();
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(*ch);
    }
    format!("{}R$ {out},{:02}", if neg { "-" } else { "" }, frac)
}

// Errors / info
pub const ERR_UNCONFIGURED: &str =
    "Supabase não configurado. Defina SUPABASE_URL e SUPABASE_ANON_KEY no .env e reconstrua.";
pub const ERR_OFFLINE: &str = "Sem conexão. Tente novamente.";
pub const ERR_BAD_CREDS: &str = "E-mail ou senha incorretos.";
pub const ERR_EMAIL_EXISTS: &str = "Este e-mail já está cadastrado.";
pub const ERR_RATE: &str = "Muitas tentativas. Aguarde e tente novamente.";
pub const ERR_SESSION: &str = "Sessão expirada. Entre novamente.";
pub const ERR_SERVER: &str = "Erro no servidor:";
pub const ERR_CONFLICT: &str = "Conflito com a versão salva.";
pub const ERR_NOT_FOUND: &str = "Registro não encontrado.";
pub const ERR_FILL_FIELDS: &str = "Preencha os campos obrigatórios.";
pub const ERR_YEAR: &str = "Ano inválido.";
pub const ERR_RENAVAM: &str = "Renavam deve ter 11 dígitos.";

pub const INFO_CONFIRM: &str = "Conta criada! Verifique seu e-mail para confirmar.";
