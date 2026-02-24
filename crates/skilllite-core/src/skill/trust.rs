use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrustTier {
    Trusted,
    Reviewed,
    Community,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrustDecision {
    Allow,
    RequireConfirm,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegritySignal {
    Ok,
    HashChanged,
    SignatureInvalid,
    Unsigned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureSignal {
    Unsigned,
    Valid,
    Invalid,
}

#[derive(Debug, Clone)]
pub struct TrustAssessment {
    pub tier: TrustTier,
    pub score: u8,
    pub reasons: Vec<String>,
    pub decision: TrustDecision,
}

pub fn assess_skill_trust(
    source: Option<&str>,
    signature: SignatureSignal,
    integrity: IntegritySignal,
    has_critical_scan: bool,
    has_high_scan: bool,
) -> TrustAssessment {
    let mut reasons = Vec::new();

    if matches!(integrity, IntegritySignal::HashChanged | IntegritySignal::SignatureInvalid)
        || matches!(signature, SignatureSignal::Invalid)
        || has_critical_scan
    {
        if matches!(integrity, IntegritySignal::HashChanged) {
            reasons.push("content hash drift detected".to_string());
        }
        if matches!(
            integrity,
            IntegritySignal::SignatureInvalid | IntegritySignal::Unsigned
        ) && matches!(signature, SignatureSignal::Invalid)
        {
            reasons.push("signature validation failed".to_string());
        }
        if has_critical_scan {
            reasons.push("critical security scan findings".to_string());
        }
        return TrustAssessment {
            tier: TrustTier::Unknown,
            score: 0,
            reasons,
            decision: TrustDecision::Deny,
        };
    }

    let mut score: i32 = 0;
    let src = source.unwrap_or("").to_lowercase();
    if src.contains("clawhub:") || src.contains("github.com/exboys/skilllite") {
        score += 25;
        reasons.push("official source".to_string());
    } else if src.contains("github.com/") || src.contains('/') {
        score += 15;
        reasons.push("known repository source".to_string());
    } else if !src.is_empty() {
        score += 8;
        reasons.push("local/custom source".to_string());
    }

    match signature {
        SignatureSignal::Valid => {
            score += 25;
            reasons.push("signature verified".to_string());
        }
        SignatureSignal::Unsigned => {
            score += 8;
            reasons.push("unsigned package".to_string());
        }
        SignatureSignal::Invalid => {}
    }

    match integrity {
        IntegritySignal::Ok => score += 20,
        IntegritySignal::Unsigned => score += 20, // hash baseline matches in manifest path
        IntegritySignal::HashChanged | IntegritySignal::SignatureInvalid => {}
    }

    if has_high_scan {
        score += 8;
        reasons.push("high-risk scan findings present".to_string());
    } else {
        score += 20;
    }

    if score > 100 {
        score = 100;
    }
    let score_u8 = score as u8;

    let (tier, decision) = if score_u8 >= 85 {
        (TrustTier::Trusted, TrustDecision::Allow)
    } else if score_u8 >= 65 {
        (TrustTier::Reviewed, TrustDecision::Allow)
    } else if score_u8 >= 40 {
        (TrustTier::Community, TrustDecision::RequireConfirm)
    } else {
        (TrustTier::Unknown, TrustDecision::RequireConfirm)
    };

    TrustAssessment {
        tier,
        score: score_u8,
        reasons,
        decision,
    }
}
