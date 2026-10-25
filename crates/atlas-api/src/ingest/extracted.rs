use atlas_domain::{
    BusinessCatalogCandidate, HostCandidate, HostTelemetryCandidate, NetworkSegmentCandidate,
    ProcessRuntimeCandidate, ProcessTelemetryCandidate, ResponsibilityAssignmentCandidate,
    ServiceEndpointCandidate, SubjectCandidate,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedServiceEndpoints {
    pub candidates: Vec<ServiceEndpointCandidate>,
    /// 输入里到底有没有 `service_endpoints` 字段。
    ///
    /// 这个区分很关键：**没这个字段**说明采集端不支持端点（macOS 上的旧探针就是这样），
    /// 不能拿它当作「这台机器一个端口都没开」去清扫；**字段是空数组**才是真的没端口。
    pub reported: bool,
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
