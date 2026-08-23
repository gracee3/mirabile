use astra_core::CanonicalResource;

use crate::RepositoryError;

pub fn resource_to_json(resource: &CanonicalResource) -> Result<String, RepositoryError> {
    resource.validate()?;
    Ok(serde_json::to_string_pretty(resource)?)
}

pub fn resource_from_json(json: &str) -> Result<CanonicalResource, RepositoryError> {
    let resource: CanonicalResource = serde_json::from_str(json)?;
    resource.validate()?;
    Ok(resource)
}

#[cfg(test)]
mod tests {
    use astra_core::{
        Angle, AspectClass, AspectDefinition, AspectId, AspectSet, CalendarSpec, ChartRecord,
        CivilDate, CivilDateTime, CivilTime, EventKind, Latitude, LocationAssertion, Longitude,
        OrbPolicy, ResourceEnvelope, SourceProvenance, SourceType, TemporalAssertion,
        TimeZoneAssertion, Timestamp,
    };

    use super::*;

    #[test]
    fn aspect_resource_round_trip_preserves_identity_revision_and_semantics() {
        let resource = CanonicalResource::AspectSet(ResourceEnvelope::new(
            "Standard aspects",
            AspectSet {
                aspects: vec![AspectDefinition {
                    id: AspectId::new("trine").expect("valid ID"),
                    name: "Trine".into(),
                    angle: Angle::from_degrees(120.0).expect("valid angle"),
                    enabled: true,
                    orbs: OrbPolicy {
                        maximum: Angle::from_degrees(7.5).expect("valid angle"),
                        applying_multiplier: 1.1,
                    },
                    classification: AspectClass::Major,
                }],
            },
            Timestamp::from_unix_millis(42),
        ));
        let json = resource_to_json(&resource).expect("serialize");
        let decoded = resource_from_json(&json).expect("deserialize");

        assert_eq!(decoded, resource);
        assert!(json.contains("aspect_set"));
        assert!(json.contains("schema_version"));
    }

    #[test]
    fn chart_record_round_trip_preserves_civil_assertion_and_coordinates() {
        let resource = CanonicalResource::ChartRecord(ResourceEnvelope::new(
            "Source assertion",
            ChartRecord {
                event_kind: EventKind::Event,
                subject: None,
                time: TemporalAssertion {
                    civil_datetime: CivilDateTime {
                        date: CivilDate::new(1985, 11, 3).expect("valid date"),
                        time: CivilTime::new(1, 30, 0).expect("valid time"),
                    },
                    calendar: CalendarSpec::ProlepticGregorian,
                    zone: TimeZoneAssertion::NamedZone("America/New_York".into()),
                    disambiguation: Some(astra_core::TimeChoice::Later),
                },
                location: LocationAssertion {
                    display_name: "New York".into(),
                    country_region: Some("US-NY".into()),
                    latitude: Latitude::from_degrees(40.7128).expect("valid latitude"),
                    longitude: Longitude::from_degrees(-74.006).expect("valid longitude"),
                    atlas_provenance: None,
                },
                source: SourceProvenance {
                    description: "User-provided test assertion".into(),
                    source_type: SourceType::UserAssertion,
                    recorded_by: None,
                },
                notes: Vec::new(),
                life_events: Vec::new(),
            },
            Timestamp::from_unix_millis(42),
        ));

        let decoded = resource_from_json(&resource_to_json(&resource).expect("serialize"))
            .expect("deserialize");
        assert_eq!(decoded, resource);
    }
}
