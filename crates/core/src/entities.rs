//! Domain entities mirroring the Supabase schema (plan.md §4).
//!
//! Conventions:
//! - Money is integer centavos (`i64`) — never floats (D11).
//! - `plate` and `state_uo` are normalized on deserialization, so any JSON
//!   that enters the app (cloud sync, fixtures, user input) is canonical.
//! - Every user-scoped entity carries `updated_at`; conflict resolution is
//!   last-write-wins by that field (D10, D15).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Fuel types (SQL: `fuel_type` check constraint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FuelType {
    Gasoline,
    Ethanol,
    Flex,
    Diesel,
    Hybrid,
    Electric,
}

impl FuelType {
    /// The pt-BR label the UI shows for a value (used by the wizard select).
    pub fn as_str(&self) -> &'static str {
        match self {
            FuelType::Gasoline => "Gasolina",
            FuelType::Ethanol => "Álcool (etanol)",
            FuelType::Flex => "Flex",
            FuelType::Diesel => "Diesel",
            FuelType::Hybrid => "Híbrido",
            FuelType::Electric => "Elétrico",
        }
    }

    /// Parse a label from the UI select (serde value is the lowercase name).
    pub fn from_str(label: &str) -> Self {
        match label {
            "gasoline" => FuelType::Gasoline,
            "ethanol" => FuelType::Ethanol,
            "flex" => FuelType::Flex,
            "diesel" => FuelType::Diesel,
            "hybrid" => FuelType::Hybrid,
            "electric" => FuelType::Electric,
            _ => FuelType::Gasoline,
        }
    }
}

/// Expense categories (SQL: `expenses.category` check constraint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpenseCategory {
    Fuel,
    Maintenance,
    Insurance,
    Tax,
    Tire,
    Fine,
    Other,
}

/// Document types (SQL: `documents.doc_type` check constraint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocType {
    Crlv,
    Insurance,
    Registration,
    Warranty,
    Other,
}

/// Deadline kinds (SQL: `deadlines.kind` check constraint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeadlineKind {
    Ipva,
    Licensing,
    Insurance,
    Inspection,
    Other,
}

/// A field-level validation failure. `field` is stable (e.g. `"plate"`) so
/// the UI can map it to the right form control; `message` is pt-BR display
/// text (D12 — strings live with the domain for now, move to `i18n` in T1-3).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{field}: {message}")]
pub struct ValidationError {
    field: &'static str,
    message: String,
}

impl ValidationError {
    pub fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    pub const fn field(&self) -> &'static str {
        self.field
    }
}

/// `vehicles` (plan.md §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vehicle {
    pub id: Uuid,
    pub user_id: Uuid,
    /// e.g. "Volkswagen"
    pub make: String,
    /// e.g. "Gol"
    pub model: String,
    /// e.g. "1.6 MSI" (optional, free text)
    pub version: Option<String>,
    /// 1950..=2030 (SQL check constraint)
    pub year: i32,
    /// Normalized uppercase, no spaces (see `crate::plate`).
    #[serde(deserialize_with = "deserialize_plate")]
    pub plate: String,
    /// Optional, 11 digits.
    pub renavam: Option<String>,
    #[serde(default = "default_fuel_type")]
    pub fuel_type: FuelType,
    /// UF of registration (2 chars, e.g. "SP"); drives the Detran deep link.
    #[serde(
        default = "default_state_uo",
        deserialize_with = "deserialize_state_uo"
    )]
    pub state_uo: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Vehicle {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.make.trim().is_empty() {
            return Err(ValidationError::new("make", "marca é obrigatória"));
        }
        if self.model.trim().is_empty() {
            return Err(ValidationError::new("model", "modelo é obrigatório"));
        }
        if !(1950..=2030).contains(&self.year) {
            return Err(ValidationError::new(
                "year",
                "ano deve estar entre 1950 e 2030",
            ));
        }
        crate::plate::validate_plate(&self.plate)?;
        if let Some(renavam) = &self.renavam {
            if renavam.len() != 11 || !renavam.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ValidationError::new(
                    "renavam",
                    "RENAVAM deve ter 11 dígitos",
                ));
            }
        }
        let uo = self.state_uo.trim();
        if uo.len() != 2 || !uo.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(ValidationError::new("state_uo", "UF deve ter 2 letras"));
        }
        Ok(())
    }
}

/// `expenses` (plan.md §4). Soft-deleted rows keep `deleted_at` set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expense {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub user_id: Uuid,
    pub category: ExpenseCategory,
    /// e.g. "Troca de óleo"
    pub title: String,
    /// BRL centavos, >= 0 (SQL check constraint).
    pub amount_cents: i64,
    pub occurred_on: NaiveDate,
    /// Optional, 0..=3_000_000 km.
    pub odometer_km: Option<i32>,
    /// Optional free text (shop name).
    pub location: Option<String>,
    pub notes: Option<String>,
    /// Soft delete.
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Expense {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.title.trim().is_empty() {
            return Err(ValidationError::new("title", "descrição é obrigatória"));
        }
        if self.amount_cents < 0 {
            return Err(ValidationError::new(
                "amount_cents",
                "valor não pode ser negativo",
            ));
        }
        if let Some(km) = self.odometer_km {
            if !(0..=3_000_000).contains(&km) {
                return Err(ValidationError::new(
                    "odometer_km",
                    "quilometragem deve estar entre 0 e 3.000.000",
                ));
            }
        }
        Ok(())
    }
}

/// `documents` (plan.md §4). Files live in the private Supabase Storage
/// bucket; the entity keeps the `storage_path` + validity window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub user_id: Uuid,
    /// e.g. "CRLV 2025"
    pub name: String,
    pub doc_type: DocType,
    /// Path in the private Supabase Storage bucket.
    pub storage_path: String,
    /// Nullable; drives the expiry chip.
    pub valid_until: Option<NaiveDate>,
    pub valid_from: Option<NaiveDate>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Document {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::new("name", "nome é obrigatório"));
        }
        if self.storage_path.trim().is_empty() {
            return Err(ValidationError::new("storage_path", "arquivo não anexado"));
        }
        if let (Some(from), Some(until)) = (self.valid_from, self.valid_until) {
            if from > until {
                return Err(ValidationError::new(
                    "valid_from",
                    "vigência não pode começar após o término",
                ));
            }
        }
        Ok(())
    }
}

/// `deadlines` (plan.md §4) — the user's known dates (IPVA, licenciamento,
/// insurance, inspection). Drives the in-app alert engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deadline {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub user_id: Uuid,
    pub kind: DeadlineKind,
    /// e.g. "IPVA 2026"
    pub label: String,
    pub due_on: NaiveDate,
    /// "Pago" toggle — silences the alert.
    #[serde(default)]
    pub paid: bool,
    /// URL to the official Detran page (prefilled per UF, user-adjustable).
    pub deep_link: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Deadline {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.label.trim().is_empty() {
            return Err(ValidationError::new("label", "descrição é obrigatória"));
        }
        if let Some(link) = &self.deep_link {
            if link.trim().is_empty() {
                return Err(ValidationError::new("deep_link", "link inválido"));
            }
        }
        Ok(())
    }
}

/// `saved_parts_searches` (plan.md §4) — a snapshot of a Mercado Livre
/// comparison for later review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPartsSearch {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub user_id: Uuid,
    /// The search query that was run.
    pub query: String,
    /// Cached top results at time of save (SQL `jsonb`).
    pub snapshot: Value,
    pub created_at: DateTime<Utc>,
}

impl SavedPartsSearch {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.query.trim().is_empty() {
            return Err(ValidationError::new("query", "busca é obrigatória"));
        }
        Ok(())
    }
}

fn default_fuel_type() -> FuelType {
    FuelType::Gasoline
}

fn default_state_uo() -> String {
    "SP".to_string()
}

fn deserialize_plate<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let raw = String::deserialize(d)?;
    Ok(crate::plate::normalize_plate(&raw))
}

fn deserialize_state_uo<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let raw = String::deserialize(d)?;
    Ok(raw.trim().to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, TimeZone};

    fn test_uuid(n: u8) -> Uuid {
        Uuid::from_u128(u128::from(n))
    }

    fn utc(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    fn vehicle() -> Vehicle {
        Vehicle {
            id: test_uuid(1),
            user_id: test_uuid(2),
            make: "Volkswagen".into(),
            model: "Gol".into(),
            version: Some("1.6 MSI".into()),
            year: 2018,
            plate: "AB1C2D3".into(),
            renavam: Some("12345678901".into()),
            fuel_type: FuelType::Flex,
            state_uo: "SP".into(),
            created_at: utc(2026, 11, 5, 12),
            updated_at: utc(2026, 11, 5, 12),
        }
    }

    #[test]
    fn vehicle_validates() {
        vehicle().validate().unwrap();
    }

    #[test]
    fn vehicle_rejects_year_out_of_range() {
        let mut v = vehicle();
        v.year = 1949;
        assert_eq!(v.validate().unwrap_err().field(), "year");
        v.year = 2031;
        assert_eq!(v.validate().unwrap_err().field(), "year");
    }

    #[test]
    fn vehicle_rejects_empty_make_or_model() {
        let mut v = vehicle();
        v.make = "   ".into();
        assert_eq!(v.validate().unwrap_err().field(), "make");
        v.make = "Volkswagen".into();
        v.model = "".into();
        assert_eq!(v.validate().unwrap_err().field(), "model");
    }

    #[test]
    fn vehicle_rejects_bad_renavam() {
        let mut v = vehicle();
        v.renavam = Some("1234567890".into()); // 10 digits
        assert_eq!(v.validate().unwrap_err().field(), "renavam");
        v.renavam = Some("1234567890A".into()); // non-digit
        assert_eq!(v.validate().unwrap_err().field(), "renavam");
    }

    #[test]
    fn vehicle_rejects_bad_state_uo() {
        let mut v = vehicle();
        v.state_uo = "S".into();
        assert_eq!(v.validate().unwrap_err().field(), "state_uo");
        v.state_uo = "ABC".into();
        assert_eq!(v.validate().unwrap_err().field(), "state_uo");
        v.state_uo = "SP1".into();
        assert_eq!(v.validate().unwrap_err().field(), "state_uo");
    }

    fn expense() -> Expense {
        Expense {
            id: test_uuid(3),
            vehicle_id: test_uuid(4),
            user_id: test_uuid(5),
            category: ExpenseCategory::Maintenance,
            title: "Troca de óleo".into(),
            amount_cents: 25000,
            occurred_on: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
            odometer_km: Some(81_230),
            location: Some("Oficina do Zé".into()),
            notes: None,
            deleted_at: None,
            created_at: utc(2026, 11, 1, 18),
            updated_at: utc(2026, 11, 1, 18),
        }
    }

    #[test]
    fn expense_validates() {
        expense().validate().unwrap();
    }

    #[test]
    fn expense_allows_zero_amount() {
        let mut e = expense();
        e.amount_cents = 0;
        e.validate().unwrap();
    }

    #[test]
    fn expense_rejects_negative_amount() {
        let mut e = expense();
        e.amount_cents = -1;
        assert_eq!(e.validate().unwrap_err().field(), "amount_cents");
    }

    #[test]
    fn expense_rejects_empty_title() {
        let mut e = expense();
        e.title = " ".into();
        assert_eq!(e.validate().unwrap_err().field(), "title");
    }

    #[test]
    fn expense_rejects_odometer_out_of_range() {
        let mut e = expense();
        e.odometer_km = Some(-5);
        assert_eq!(e.validate().unwrap_err().field(), "odometer_km");
        e.odometer_km = Some(3_000_001);
        assert_eq!(e.validate().unwrap_err().field(), "odometer_km");
        e.odometer_km = Some(3_000_000); // boundary passes
        e.validate().unwrap();
    }

    fn document() -> Document {
        Document {
            id: test_uuid(6),
            vehicle_id: test_uuid(7),
            user_id: test_uuid(8),
            name: "CRLV 2025".into(),
            doc_type: DocType::Crlv,
            storage_path: "documents/ab1c2d3/crlv-2025.pdf".into(),
            valid_until: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            valid_from: None,
            deleted_at: None,
            created_at: utc(2026, 11, 2, 10),
            updated_at: utc(2026, 11, 2, 10),
        }
    }

    #[test]
    fn document_validates() {
        document().validate().unwrap();
    }

    #[test]
    fn document_rejects_empty_name_or_path() {
        let mut d = document();
        d.name = "".into();
        assert_eq!(d.validate().unwrap_err().field(), "name");
        d.name = "CRLV 2025".into();
        d.storage_path = " ".into();
        assert_eq!(d.validate().unwrap_err().field(), "storage_path");
    }

    #[test]
    fn document_rejects_inverted_validity_window() {
        let mut d = document();
        d.valid_from = Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
        d.valid_until = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(d.validate().unwrap_err().field(), "valid_from");
    }

    fn deadline() -> Deadline {
        Deadline {
            id: test_uuid(9),
            vehicle_id: test_uuid(10),
            user_id: test_uuid(11),
            kind: DeadlineKind::Ipva,
            label: "IPVA 2026".into(),
            due_on: NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(),
            paid: false,
            deep_link: Some("https://www.detran.sp.gov.br".into()),
            deleted_at: None,
            created_at: utc(2026, 11, 3, 9),
            updated_at: utc(2026, 11, 3, 9),
        }
    }

    #[test]
    fn deadline_validates() {
        deadline().validate().unwrap();
    }

    #[test]
    fn deadline_rejects_empty_label_or_blank_link() {
        let mut d = deadline();
        d.label = "".into();
        assert_eq!(d.validate().unwrap_err().field(), "label");
        d.label = "IPVA 2026".into();
        d.deep_link = Some("  ".into());
        assert_eq!(d.validate().unwrap_err().field(), "deep_link");
    }

    fn saved_search() -> SavedPartsSearch {
        SavedPartsSearch {
            id: test_uuid(12),
            vehicle_id: test_uuid(13),
            user_id: test_uuid(14),
            query: "pastilha de freio Volkswagen Gol 1.6".into(),
            snapshot: serde_json::json!([
                { "id": "MLB1", "title": "Pastilha dianteira", "price_cents": 12990 },
                { "id": "MLB2", "title": "Kit pastilhas", "price_cents": null }
            ]),
            created_at: utc(2026, 11, 4, 20),
        }
    }

    #[test]
    fn saved_search_validates() {
        saved_search().validate().unwrap();
    }

    #[test]
    fn saved_search_rejects_empty_query() {
        let mut s = saved_search();
        s.query = " ".into();
        assert_eq!(s.validate().unwrap_err().field(), "query");
    }

    #[test]
    fn enums_serialize_to_sql_values() {
        assert_eq!(serde_json::to_value(FuelType::Flex).unwrap(), "flex");
        assert_eq!(
            serde_json::to_value(ExpenseCategory::Maintenance).unwrap(),
            "maintenance"
        );
        assert_eq!(serde_json::to_value(DocType::Crlv).unwrap(), "crlv");
        assert_eq!(serde_json::to_value(DeadlineKind::Ipva).unwrap(), "ipva");
        // unknown values must fail (SQL would reject them)
        assert!(serde_json::from_str::<FuelType>("\"natural_gas\"").is_err());
        assert!(serde_json::from_str::<ExpenseCategory>("\"coffee\"").is_err());
    }

    #[test]
    fn vehicle_defaults_when_optional_fields_absent() {
        let v: Vehicle = serde_json::from_value(serde_json::json!({
            "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "user_id": "6ba7b811-9dad-11d1-80b4-00c04fd430c8",
            "make": "Fiat",
            "model": "Uno",
            "year": 2005,
            "plate": "AB12C34",
            "created_at": "2026-11-05T12:00:00Z",
            "updated_at": "2026-11-05T12:00:00Z"
        }))
        .unwrap();
        assert_eq!(v.fuel_type, FuelType::Gasoline);
        assert_eq!(v.state_uo, "SP");
        assert!(v.version.is_none());
        assert!(v.renavam.is_none());
    }

    #[test]
    fn plate_normalized_on_deserialize() {
        let v: Vehicle = serde_json::from_value(serde_json::json!({
            "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "user_id": "6ba7b811-9dad-11d1-80b4-00c04fd430c8",
            "make": "Fiat",
            "model": "Uno",
            "year": 2005,
            "plate": "  ab 12c34 ",
            "state_uo": "sp",
            "created_at": "2026-11-05T12:00:00Z",
            "updated_at": "2026-11-05T12:00:00Z"
        }))
        .unwrap();
        assert_eq!(v.plate, "AB12C34");
        assert_eq!(v.state_uo, "SP");
    }
}
