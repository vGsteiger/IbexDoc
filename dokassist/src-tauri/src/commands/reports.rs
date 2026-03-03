use crate::audit::{self, AuditAction};
use crate::error::AppError;
use crate::models::report::{self, CreateReport, Report, UpdateReport};
use crate::models::patient;
use crate::state::AppState;
use tauri::State;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

#[tauri::command]
pub async fn create_report(
    state: State<'_, AppState>,
    input: CreateReport,
) -> Result<Report, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    let tx = conn.unchecked_transaction()?;

    let report = report::create_report(&tx, input)?;

    // PKG-6: Audit logging
    audit::log(&tx, AuditAction::Create, "report", Some(&report.id), None)?;

    tx.commit()?;

    Ok(report)
}

#[tauri::command]
pub async fn get_report(state: State<'_, AppState>, id: String) -> Result<Report, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;
    let report = report::get_report(&conn, &id)?;

    // PKG-6: Audit logging
    audit::log(&conn, AuditAction::View, "report", Some(&id), None)?;

    Ok(report)
}

#[tauri::command]
pub async fn list_reports(
    state: State<'_, AppState>,
    patient_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Report>, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    let reports = report::list_reports_for_patient(&conn, &patient_id, limit, offset)?;

    Ok(reports)
}

#[tauri::command]
pub async fn update_report(
    state: State<'_, AppState>,
    id: String,
    input: UpdateReport,
) -> Result<Report, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    let tx = conn.unchecked_transaction()?;

    let report = report::update_report(&tx, &id, input)?;

    // PKG-6: Audit logging
    audit::log(&tx, AuditAction::Update, "report", Some(&id), None)?;

    tx.commit()?;

    Ok(report)
}

#[tauri::command]
pub async fn delete_report(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    let tx = conn.unchecked_transaction()?;

    report::delete_report(&tx, &id)?;

    // PKG-6: Audit logging
    audit::log(&tx, AuditAction::Delete, "report", Some(&id), None)?;

    tx.commit()?;

    Ok(())
}

#[tauri::command]
pub async fn export_report_to_pdf(
    state: State<'_, AppState>,
    id: String,
    output_path: String,
) -> Result<(), AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    // Get the report
    let report = report::get_report(&conn, &id)?;

    // Get patient information
    let patient = patient::get_patient(&conn, &report.patient_id)?;

    // Create PDF document
    let (doc, page1, layer1) = PdfDocument::new(
        "Report",
        Mm(210.0), // A4 width
        Mm(297.0), // A4 height
        "Layer 1"
    );

    let current_layer = doc.get_page(page1).get_layer(layer1);

    // Load a standard font
    let font = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| {
        AppError::Internal(format!("Failed to load font: {}", e))
    })?;
    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold).map_err(|e| {
        AppError::Internal(format!("Failed to load font: {}", e))
    })?;

    // Set initial position
    let mut current_y = Mm(270.0); // Start near top of page
    let left_margin = Mm(20.0);
    let line_height = Mm(5.0);

    // Add header
    current_layer.use_text(
        format!("Report: {}", report.report_type),
        18.0,
        left_margin,
        current_y,
        &font_bold
    );
    current_y = current_y - line_height * 2.0;

    // Add patient information
    current_layer.use_text(
        "Patient Information",
        14.0,
        left_margin,
        current_y,
        &font_bold
    );
    current_y = current_y - line_height;

    current_layer.use_text(
        format!("Name: {} {}", patient.first_name, patient.last_name),
        10.0,
        left_margin,
        current_y,
        &font
    );
    current_y = current_y - line_height;

    current_layer.use_text(
        format!("Date of Birth: {}", patient.date_of_birth),
        10.0,
        left_margin,
        current_y,
        &font
    );
    current_y = current_y - line_height;

    if let Some(address) = &patient.address {
        current_layer.use_text(
            format!("Address: {}", address),
            10.0,
            left_margin,
            current_y,
            &font
        );
        current_y = current_y - line_height;
    }

    current_y = current_y - line_height;

    // Add report metadata
    current_layer.use_text(
        format!("Generated: {}", report.generated_at),
        10.0,
        left_margin,
        current_y,
        &font
    );
    current_y = current_y - line_height;

    if let Some(model_name) = &report.model_name {
        current_layer.use_text(
            format!("Model: {}", model_name),
            10.0,
            left_margin,
            current_y,
            &font
        );
        current_y = current_y - line_height;
    }

    current_y = current_y - line_height;

    // Add report content header
    current_layer.use_text(
        "Report Content",
        14.0,
        left_margin,
        current_y,
        &font_bold
    );
    current_y = current_y - line_height * 1.5;

    // Add report content (simple line wrapping)
    let max_chars_per_line = 90;
    let content_lines: Vec<&str> = report.content.lines().collect();

    for line in content_lines {
        if line.trim().is_empty() {
            current_y = current_y - line_height * 0.5;
            continue;
        }

        // Simple word wrapping
        let mut current_line = String::new();
        for word in line.split_whitespace() {
            if current_line.len() + word.len() + 1 > max_chars_per_line {
                // Print current line and start new one
                if current_y < Mm(30.0) {
                    // Need new page
                    let (page, layer) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
                    let current_layer = doc.get_page(page).get_layer(layer);
                    current_y = Mm(270.0);

                    current_layer.use_text(
                        &current_line,
                        10.0,
                        left_margin,
                        current_y,
                        &font
                    );
                } else {
                    current_layer.use_text(
                        &current_line,
                        10.0,
                        left_margin,
                        current_y,
                        &font
                    );
                }
                current_y = current_y - line_height;
                current_line = word.to_string();
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
            }
        }

        // Print remaining text
        if !current_line.is_empty() {
            if current_y < Mm(30.0) {
                let (page, layer) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
                let current_layer = doc.get_page(page).get_layer(layer);
                current_y = Mm(270.0);

                current_layer.use_text(
                    &current_line,
                    10.0,
                    left_margin,
                    current_y,
                    &font
                );
            } else {
                current_layer.use_text(
                    &current_line,
                    10.0,
                    left_margin,
                    current_y,
                    &font
                );
            }
            current_y = current_y - line_height;
        }
    }

    // Save PDF
    let file = File::create(&output_path).map_err(|e| {
        AppError::Internal(format!("Failed to create PDF file: {}", e))
    })?;
    let buf_writer = BufWriter::new(file);
    doc.save(&mut buf_writer).map_err(|e| {
        AppError::Internal(format!("Failed to save PDF: {}", e))
    })?;

    // PKG-6: Audit logging
    audit::log(&conn, AuditAction::View, "report", Some(&id), Some(&format!("Exported to PDF: {}", output_path)))?;

    Ok(())
}
