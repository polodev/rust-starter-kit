//! A sample invoice: data, totals and the HTML template. It shows how to make a PDF from a template and is
//! what `pdf:sample` and `mail:send-invoice` use.

use serde::Serialize;
use serde_json::{json, Value};

use crate::{html_to_pdf, render_template, PdfOptions};
use loco_rs::Result;

const TEMPLATE: &str = include_str!("../templates/pdf/invoice.html");

#[derive(Debug, Clone, Serialize)]
pub struct Party {
    pub name: String,
    pub address: String,
    pub email: String,
}

/// One line of the invoice. Money is in minor units (poisha, cents) so nothing is lost to rounding.
#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub description: String,
    pub quantity: u32,
    pub unit_price_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Invoice {
    pub number: String,
    pub issued: String,
    pub due: String,
    pub paid: bool,
    pub currency: String,
    pub seller: Party,
    pub customer: Party,
    pub lines: Vec<Line>,
    pub tax_label: String,
    /// Tax in hundredths of a percent: 1500 is 15%.
    pub tax_basis_points: i64,
    pub note: Option<String>,
    pub footer: String,
}

/// `1234567` minor units as `12,345.67`.
pub fn money(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let minor = minor.unsigned_abs();
    let (whole, cents) = (minor / 100, minor % 100);
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("{sign}{grouped}.{cents:02}")
}

impl Invoice {
    pub fn subtotal_minor(&self) -> i64 {
        self.lines
            .iter()
            .map(|line| i64::from(line.quantity) * line.unit_price_minor)
            .sum()
    }

    /// Tax rounded half up to a whole minor unit.
    pub fn tax_minor(&self) -> i64 {
        let tax = i128::from(self.subtotal_minor()) * i128::from(self.tax_basis_points);
        ((tax + 5_000) / 10_000) as i64
    }

    pub fn total_minor(&self) -> i64 {
        self.subtotal_minor() + self.tax_minor()
    }

    /// The values the template uses, with money already formatted.
    pub fn template_data(&self) -> Value {
        let lines: Vec<Value> = self
            .lines
            .iter()
            .map(|line| {
                json!({
                    "description": line.description, "quantity": line.quantity,
                    "unit_price": money(line.unit_price_minor),
                    "amount": money(i64::from(line.quantity) * line.unit_price_minor),
                })
            })
            .collect();
        json!({
            "number": self.number, "issued": self.issued, "due": self.due, "paid": self.paid,
            "currency": self.currency, "seller": self.seller, "customer": self.customer,
            "items": lines, "subtotal": money(self.subtotal_minor()), "tax": money(self.tax_minor()),
            "tax_label": self.tax_label, "tax_percent": format!("{}", self.tax_basis_points as f64 / 100.0),
            "total": money(self.total_minor()), "note": self.note, "footer": self.footer,
        })
    }

    pub fn html(&self) -> Result<String> {
        render_template(TEMPLATE, &self.template_data())
    }

    pub async fn pdf(&self, options: &PdfOptions) -> Result<Vec<u8>> {
        html_to_pdf(&self.html()?, options).await
    }

    /// A made-up invoice for trying things out. It includes Bangla text to prove that it prints correctly.
    pub fn sample(customer_email: &str) -> Self {
        let today = chrono::Utc::now().date_naive();
        Self {
            number: format!("INV-{}", today.format("%Y%m%d-001")),
            issued: today.format("%d %b %Y").to_string(),
            due: (today + chrono::Duration::days(14))
                .format("%d %b %Y")
                .to_string(),
            paid: false,
            currency: "BDT".to_string(),
            seller: Party {
                name: "Rust Starter".to_string(),
                address: "House 12, Road 5, Dhaka 1207, Bangladesh".to_string(),
                email: "billing@example.com".to_string(),
            },
            customer: Party {
                name: "Rahim Uddin (রহিম উদ্দিন)".to_string(),
                address: "Flat 4B, Mirpur DOHS, ঢাকা, বাংলাদেশ".to_string(),
                email: customer_email.to_string(),
            },
            lines: vec![
                Line {
                    description: "Website development (ওয়েবসাইট তৈরি)".to_string(),
                    quantity: 1,
                    unit_price_minor: 4_500_000,
                },
                Line {
                    description: "Monthly hosting and backups (হোস্টিং ও ব্যাকআপ)".to_string(),
                    quantity: 3,
                    unit_price_minor: 150_000,
                },
                Line {
                    description: "Support hours (সাপোর্ট)".to_string(),
                    quantity: 5,
                    unit_price_minor: 80_000,
                },
            ],
            tax_label: "VAT".to_string(),
            tax_basis_points: 1_500,
            note: Some(
                "Thank you for your business. ধন্যবাদ! Payment is due within 14 days.".to_string(),
            ),
            footer: "Generated by Rust Starter with headless Chrome".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_is_grouped_and_never_loses_a_poisha() {
        assert_eq!(money(0), "0.00");
        assert_eq!(money(5), "0.05");
        assert_eq!(money(123_456_789), "1,234,567.89");
        assert_eq!(money(-250), "-2.50");
    }

    #[test]
    fn totals_and_rounding_are_exact() {
        let invoice = Invoice::sample("x@example.com");
        assert_eq!(invoice.subtotal_minor(), 4_500_000 + 450_000 + 400_000);
        assert_eq!(invoice.tax_minor(), 802_500, "15% of 53,500.00");
        assert_eq!(invoice.total_minor(), 6_152_500);
        let mut odd = invoice.clone();
        odd.lines = vec![Line {
            description: "x".into(),
            quantity: 1,
            unit_price_minor: 333,
        }];
        odd.tax_basis_points = 1_500;
        assert_eq!(odd.tax_minor(), 50, "49.95 rounds half up to 50");
    }

    #[test]
    fn the_html_has_the_numbers_and_the_bangla_text() {
        let html = Invoice::sample("customer@example.com").html().unwrap();
        for needle in [
            "INV-",
            "61,525.00",
            "ওয়েবসাইট তৈরি",
            "customer@example.com",
            "VAT (15%)",
            "DUE",
        ] {
            assert!(html.contains(needle), "missing {needle}");
        }
    }
}
