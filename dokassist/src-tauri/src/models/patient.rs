use crate::ahv::validate_ahv;
use crate::error::AppError;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    pub id: String,
    pub ahv_number: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: String,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub insurance: Option<String>,
    pub gp_name: Option<String>,
    pub gp_address: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePatient {
    pub ahv_number: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: String,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub insurance: Option<String>,
    pub gp_name: Option<String>,
    pub gp_address: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePatient {
    pub ahv_number: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub insurance: Option<String>,
    pub gp_name: Option<String>,
    pub gp_address: Option<String>,
    pub notes: Option<String>,
}

fn row_to_patient(row: &Row) -> Result<Patient, rusqlite::Error> {
    Ok(Patient {
        id: row.get(0)?,
        ahv_number: row.get(1)?,
        first_name: row.get(2)?,
        last_name: row.get(3)?,
        date_of_birth: row.get(4)?,
        gender: row.get(5)?,
        address: row.get(6)?,
        phone: row.get(7)?,
        email: row.get(8)?,
        insurance: row.get(9)?,
        gp_name: row.get(10)?,
        gp_address: row.get(11)?,
        notes: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

pub fn create_patient(conn: &Connection, input: CreatePatient) -> Result<Patient, AppError> {
    // Validate and normalize AHV number
    let normalized_ahv = validate_ahv(&input.ahv_number)?;

    // Generate UUIDv7 (time-sortable)
    let id = Uuid::now_v7().to_string();

    conn.execute(
        "INSERT INTO patients (
            id, ahv_number, first_name, last_name, date_of_birth,
            gender, address, phone, email, insurance, gp_name, gp_address, notes
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            id,
            normalized_ahv,
            input.first_name,
            input.last_name,
            input.date_of_birth,
            input.gender,
            input.address,
            input.phone,
            input.email,
            input.insurance,
            input.gp_name,
            input.gp_address,
            input.notes,
        ],
    )?;

    get_patient(conn, &id)
}

pub fn get_patient(conn: &Connection, id: &str) -> Result<Patient, AppError> {
    let patient = conn.query_row(
        "SELECT id, ahv_number, first_name, last_name, date_of_birth,
                gender, address, phone, email, insurance, gp_name, gp_address, notes,
                created_at, updated_at
         FROM patients WHERE id = ?",
        params![id],
        row_to_patient,
    )?;

    Ok(patient)
}

pub fn update_patient(
    conn: &Connection,
    id: &str,
    input: UpdatePatient,
) -> Result<Patient, AppError> {
    // Check if patient exists
    get_patient(conn, id)?;

    // Validate AHV if provided
    let normalized_ahv = if let Some(ref ahv) = input.ahv_number {
        Some(validate_ahv(ahv)?)
    } else {
        None
    };

    // Build dynamic UPDATE query based on provided fields
    let mut updates = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ahv) = normalized_ahv {
        updates.push("ahv_number = ?");
        values.push(Box::new(ahv));
    }
    if let Some(first_name) = input.first_name {
        updates.push("first_name = ?");
        values.push(Box::new(first_name));
    }
    if let Some(last_name) = input.last_name {
        updates.push("last_name = ?");
        values.push(Box::new(last_name));
    }
    if let Some(date_of_birth) = input.date_of_birth {
        updates.push("date_of_birth = ?");
        values.push(Box::new(date_of_birth));
    }
    if let Some(gender) = input.gender {
        updates.push("gender = ?");
        values.push(Box::new(gender));
    }
    if let Some(address) = input.address {
        updates.push("address = ?");
        values.push(Box::new(address));
    }
    if let Some(phone) = input.phone {
        updates.push("phone = ?");
        values.push(Box::new(phone));
    }
    if let Some(email) = input.email {
        updates.push("email = ?");
        values.push(Box::new(email));
    }
    if let Some(insurance) = input.insurance {
        updates.push("insurance = ?");
        values.push(Box::new(insurance));
    }
    if let Some(gp_name) = input.gp_name {
        updates.push("gp_name = ?");
        values.push(Box::new(gp_name));
    }
    if let Some(gp_address) = input.gp_address {
        updates.push("gp_address = ?");
        values.push(Box::new(gp_address));
    }
    if let Some(notes) = input.notes {
        updates.push("notes = ?");
        values.push(Box::new(notes));
    }

    if updates.is_empty() {
        return get_patient(conn, id);
    }

    let query = format!("UPDATE patients SET {} WHERE id = ?", updates.join(", "));
    values.push(Box::new(id.to_string()));

    let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    conn.execute(&query, params.as_slice())?;

    get_patient(conn, id)
}

pub fn delete_patient(conn: &Connection, id: &str) -> Result<(), AppError> {
    let rows_affected = conn.execute("DELETE FROM patients WHERE id = ?", params![id])?;

    if rows_affected == 0 {
        return Err(AppError::NotFound(format!("Patient not found: {}", id)));
    }

    Ok(())
}

pub fn list_patients(conn: &Connection, limit: u32, offset: u32) -> Result<Vec<Patient>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, ahv_number, first_name, last_name, date_of_birth,
                gender, address, phone, email, insurance, gp_name, gp_address, notes,
                created_at, updated_at
         FROM patients
         ORDER BY last_name, first_name
         LIMIT ? OFFSET ?",
    )?;

    let patients = stmt
        .query_map(params![limit, offset], row_to_patient)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(patients)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_db;
    use tempfile::tempdir;

    fn setup_test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let key = crate::crypto::generate_key();
        let pool = init_db(&db_path, &key).unwrap();
        let conn = pool.conn().unwrap();
        // Return both dir and connection (dir must stay alive)
        // We need to extract the connection from the MutexGuard
        // For testing, let's create a new connection directly
        let conn = Connection::open(&db_path).unwrap();
        let key_hex = hex::encode(&key);
        conn.execute(&format!("PRAGMA key = \"x'{}'\";", key_hex), [])
            .unwrap();
        (dir, conn)
    }

    #[test]
    fn test_create_and_get_patient() {
        let (_dir, conn) = setup_test_db();

        let input = CreatePatient {
            ahv_number: "7561234567897".to_string(),
            first_name: "Hans".to_string(),
            last_name: "Müller".to_string(),
            date_of_birth: "1980-01-15".to_string(),
            gender: Some("male".to_string()),
            address: None,
            phone: None,
            email: None,
            insurance: None,
            gp_name: None,
            gp_address: None,
            notes: None,
        };

        let patient = create_patient(&conn, input).unwrap();
        assert_eq!(patient.ahv_number, "756.1234.5678.97");
        assert_eq!(patient.first_name, "Hans");
        assert_eq!(patient.last_name, "Müller");

        let retrieved = get_patient(&conn, &patient.id).unwrap();
        assert_eq!(retrieved.id, patient.id);
    }

    #[test]
    fn test_update_patient() {
        let (_dir, conn) = setup_test_db();

        let input = CreatePatient {
            ahv_number: "7561234567897".to_string(),
            first_name: "Hans".to_string(),
            last_name: "Müller".to_string(),
            date_of_birth: "1980-01-15".to_string(),
            gender: Some("male".to_string()),
            address: None,
            phone: None,
            email: None,
            insurance: None,
            gp_name: None,
            gp_address: None,
            notes: None,
        };

        let patient = create_patient(&conn, input).unwrap();

        let update = UpdatePatient {
            first_name: Some("Peter".to_string()),
            phone: Some("+41791234567".to_string()),
            ..Default::default()
        };

        let updated = update_patient(&conn, &patient.id, update).unwrap();
        assert_eq!(updated.first_name, "Peter");
        assert_eq!(updated.phone, Some("+41791234567".to_string()));
        assert_eq!(updated.last_name, "Müller"); // unchanged
    }

    #[test]
    fn test_delete_patient() {
        let (_dir, conn) = setup_test_db();

        let input = CreatePatient {
            ahv_number: "7561234567897".to_string(),
            first_name: "Hans".to_string(),
            last_name: "Müller".to_string(),
            date_of_birth: "1980-01-15".to_string(),
            gender: None,
            address: None,
            phone: None,
            email: None,
            insurance: None,
            gp_name: None,
            gp_address: None,
            notes: None,
        };

        let patient = create_patient(&conn, input).unwrap();
        delete_patient(&conn, &patient.id).unwrap();

        let result = get_patient(&conn, &patient.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_patients() {
        let (_dir, conn) = setup_test_db();

        // Create multiple patients
        for i in 0..5 {
            let input = CreatePatient {
                ahv_number: format!("756000000001{}", i),
                first_name: format!("Test{}", i),
                last_name: format!("User{}", i),
                date_of_birth: "1980-01-01".to_string(),
                gender: None,
                address: None,
                phone: None,
                email: None,
                insurance: None,
                gp_name: None,
                gp_address: None,
                notes: None,
            };
            create_patient(&conn, input).unwrap();
        }

        let patients = list_patients(&conn, 10, 0).unwrap();
        assert_eq!(patients.len(), 5);
    }
}

impl Default for UpdatePatient {
    fn default() -> Self {
        Self {
            ahv_number: None,
            first_name: None,
            last_name: None,
            date_of_birth: None,
            gender: None,
            address: None,
            phone: None,
            email: None,
            insurance: None,
            gp_name: None,
            gp_address: None,
            notes: None,
        }
    }
}
