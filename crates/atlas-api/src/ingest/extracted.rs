use atlas_domain::{
    BusinessCatalogCandidate, HostCandidate, HostTelemetryCandidate, NetworkSegmentCandidate,
    ProcessRuntimeCandidate, ProcessTelemetryCandidate, ResponsibilityAssignmentCandidate,
    SubjectCandidate,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedBusinessCatalog {
    pub candidates: Vec<BusinessCatalogCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedHosts {
    pub candidates: Vec<HostCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedNetworkSegments {
    pub candidates: Vec<NetworkSegmentCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedProcessRuntimes {
    pub candidates: Vec<ProcessRuntimeCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedHostTelemetry {
    pub candidates: Vec<HostTelemetryCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedProcessTelemetry {
    pub candidates: Vec<ProcessTelemetryCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedSubjects {
    pub candidates: Vec<SubjectCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedResponsibilityAssignments {
    pub candidates: Vec<ResponsibilityAssignmentCandidate>,
}
