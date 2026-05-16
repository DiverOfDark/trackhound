use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "snake_case")]
pub enum ShipmentStatus {
    Detected,
    Registered,
    InTransit,
    OutForDelivery,
    Delivered,
    Failed,
    Unknown,
}

impl ShipmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ShipmentStatus::Detected => "detected",
            ShipmentStatus::Registered => "registered",
            ShipmentStatus::InTransit => "in_transit",
            ShipmentStatus::OutForDelivery => "out_for_delivery",
            ShipmentStatus::Delivered => "delivered",
            ShipmentStatus::Failed => "failed",
            ShipmentStatus::Unknown => "unknown",
        }
    }

    pub fn from_text(text: &str) -> Self {
        let t = text.to_lowercase();
        if t.contains("out for delivery")
            || t.contains("out-for-delivery")
            || t.contains("zur zustellung")
        {
            ShipmentStatus::OutForDelivery
        } else if t.contains("delivered") || t.contains("zugestellt") {
            ShipmentStatus::Delivered
        } else if t.contains("failed")
            || t.contains("exception")
            || t.contains("undelivered")
            || t.contains("alert")
        {
            ShipmentStatus::Failed
        } else if t.contains("transit")
            || t.contains("arrived")
            || t.contains("departed")
            || t.contains("processed")
        {
            ShipmentStatus::InTransit
        } else if t.contains("registered") || t.contains("label created") {
            ShipmentStatus::Registered
        } else {
            ShipmentStatus::Unknown
        }
    }

    pub fn from_17track_status(code: Option<i64>, text: Option<&str>) -> Self {
        match code {
            Some(40) => ShipmentStatus::Delivered,
            Some(35) | Some(50) => ShipmentStatus::Failed,
            Some(30) => ShipmentStatus::OutForDelivery,
            Some(10) => ShipmentStatus::InTransit,
            Some(0) | Some(20) => ShipmentStatus::Unknown,
            _ => text.map(Self::from_text).unwrap_or(ShipmentStatus::Unknown),
        }
    }
}

impl fmt::Display for ShipmentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for ShipmentStatus {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<&str> for ShipmentStatus {
    fn from(value: &str) -> Self {
        match value {
            "detected" => ShipmentStatus::Detected,
            "registered" => ShipmentStatus::Registered,
            "in_transit" => ShipmentStatus::InTransit,
            "out_for_delivery" => ShipmentStatus::OutForDelivery,
            "delivered" => ShipmentStatus::Delivered,
            "failed" => ShipmentStatus::Failed,
            _ => ShipmentStatus::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_17track_codes_to_statuses() {
        assert_eq!(
            ShipmentStatus::from_17track_status(Some(40), None),
            ShipmentStatus::Delivered
        );
        assert_eq!(
            ShipmentStatus::from_17track_status(Some(10), None),
            ShipmentStatus::InTransit
        );
        assert_eq!(
            ShipmentStatus::from_17track_status(Some(30), None),
            ShipmentStatus::OutForDelivery
        );
        assert_eq!(
            ShipmentStatus::from_17track_status(Some(35), None),
            ShipmentStatus::Failed
        );
    }

    #[test]
    fn detects_out_for_delivery_from_text() {
        assert_eq!(
            ShipmentStatus::from_text("Your parcel is out for delivery today"),
            ShipmentStatus::OutForDelivery
        );
    }
}
