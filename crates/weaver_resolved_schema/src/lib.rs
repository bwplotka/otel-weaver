// SPDX-License-Identifier: Apache-2.0

//! Define the concept of Resolved Telemetry Schema.
//!
//! A Resolved Telemetry Schema is self-contained and doesn't contain any
//! external references to other schemas or semantic conventions.

use std::any::Any;
use crate::attribute::Attribute;
use crate::catalog::Catalog;
use crate::instrumentation_library::InstrumentationLibrary;
use crate::registry::{Group, Registry};
use crate::resource::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::deprecated::{Deprecated, DeprecatedUpdated};
use weaver_semconv::group::{is_simple_mode_enabled, GroupType};
use weaver_semconv::manifest::RegistryManifest;
use weaver_version::schema_changes::{SchemaChanges, SchemaItemChange, SchemaItemChangeRenamed, SchemaItemChangeUpdated, SchemaItemType};
use weaver_version::Versions;
use std::collections::HashMap;
use weaver_semconv::attribute::AttributeType;

pub mod attribute;
pub mod catalog;
pub mod error;
pub mod instrumentation_library;
pub mod lineage;
pub mod metric;
pub mod registry;
pub mod resource;
pub mod signal;
pub mod tags;
pub mod value;

/// The registry ID for the OpenTelemetry semantic conventions.
/// This ID is reserved and should not be used by any other registry.
pub const OTEL_REGISTRY_ID: &str = "OTEL";

/// A Resolved Telemetry Schema.
/// A Resolved Telemetry Schema is self-contained and doesn't contain any
/// external references to other schemas or semantic conventions.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTelemetrySchema {
    /// Version of the file structure.
    pub file_format: String,
    /// Schema URL that this file is published at.
    pub schema_url: String,
    /// The ID of the registry that this schema belongs to.
    pub registry_id: String,
    /// The registry that this schema belongs to.
    pub registry: Registry,
    /// Catalog of unique items that are shared across multiple registries
    /// and signals.
    pub catalog: Catalog,
    /// Resource definition (only for application).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Resource>,
    /// Definition of the instrumentation library for the instrumented application or library.
    /// Or none if the resolved telemetry schema represents a semantic convention registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrumentation_library: Option<InstrumentationLibrary>,
    /// The list of dependencies of the current instrumentation application or library.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<InstrumentationLibrary>,
    /// Definitions for each schema version in this family.
    /// Note: the ordering of versions is defined according to semver
    /// version number ordering rules.
    /// This section is described in more details in the OTEP 0152 and in a dedicated
    /// section below.
    /// <https://github.com/open-telemetry/oteps/blob/main/text/0152-telemetry-schemas.md>
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Versions>,
    /// The manifest of the registry.
    pub registry_manifest: Option<RegistryManifest>,
}

/// Statistics on a resolved telemetry schema.
#[derive(Debug, Serialize)]
#[must_use]
pub struct Stats {
    /// Statistics on each registry.
    pub registry_stats: Vec<registry::Stats>,
    /// Statistics on the catalog.
    pub catalog_stats: catalog::Stats,
}

impl ResolvedTelemetrySchema {
    /// Create a new resolved telemetry schema.
    pub fn new<S: AsRef<str>>(
        file_format: S,
        schema_url: S,
        registry_id: S,
        registry_url: S,
    ) -> Self {
        Self {
            file_format: file_format.as_ref().to_owned(),
            schema_url: schema_url.as_ref().to_owned(),
            registry_id: registry_id.as_ref().to_owned(),
            registry: Registry::new(registry_url),
            catalog: Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: vec![],
            versions: None,
            registry_manifest: None,
        }
    }

    /// Adds a new attribute group to the schema.
    ///
    /// Note: This method is intended to be used for testing purposes only.
    #[cfg(test)]
    pub(crate) fn add_attribute_group<const N: usize>(
        &mut self,
        group_id: &str,
        attrs: [Attribute; N],
    ) {
        let attr_refs = self.catalog.add_attributes(attrs);
        self.registry.groups.push(Group {
            id: group_id.to_owned(),
            r#type: GroupType::AttributeGroup,
            brief: "".to_owned(),
            note: "".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: None,
            deprecated: None,
            name: Some(group_id.to_owned()),
            lineage: None,
            display_name: None,
            attributes: attr_refs,
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            constraints: vec![],
            unit: None,
            body: None,
        });
    }

    /// Get the catalog of the resolved telemetry schema.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Compute statistics on the resolved telemetry schema.
    pub fn stats(&self) -> Stats {
        let registry_stats = vec![self.registry.stats()];
        Stats {
            registry_stats,
            catalog_stats: self.catalog.stats(),
        }
    }

    /// Get the attributes of the resolved telemetry schema.
    #[must_use]
    pub fn attribute_map(&self) -> HashMap<&str, &Attribute> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.r#type == GroupType::AttributeGroup)
            .flat_map(|group| {
                group.attributes.iter().map(|attr_ref| {
                    // An attribute ref is a reference to an attribute in the catalog.
                    // Not finding the attribute in the catalog is a bug somewhere in
                    // the resolution process. So it's fine to panic here.
                    let attr = self
                        .catalog
                        .attribute(attr_ref)
                        .expect("Attribute ref not found in catalog. This is a bug.");
                    (attr.name.as_str(), attr)
                })
            })
            .collect()
    }

    /// Get the "registry" attributes of the resolved telemetry schema.
    ///
    /// Note: At the moment (2024-12-30), I don't know a better way to identify
    /// the "registry" attributes other than by checking if the group ID starts
    /// with "registry.".
    #[must_use]
    pub fn registry_attribute_map(&self) -> HashMap<&str, &Attribute> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.is_registry_attribute_group())
            .flat_map(|group| {
                group.attributes.iter().map(|attr_ref| {
                    // An attribute ref is a reference to an attribute in the catalog.
                    // Not finding the attribute in the catalog is a bug somewhere in
                    // the resolution process. So it's fine to panic here.
                    let attr = self
                        .catalog
                        .attribute(attr_ref)
                        .expect("Attribute ref not found in catalog. This is a bug.");
                    (attr.name.as_str(), attr)
                })
            })
            .collect()
    }

    /// Get the groups of a specific type from the resolved telemetry schema.
    #[must_use]
    pub fn groups(&self, group_type: GroupType) -> HashMap<&str, &Group> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.r#type == group_type)
            .map(|group| (group.id.as_str(), group))
            .collect()
    }

    /// Generate a diff between the current schema (must be the most recent one)
    /// and a baseline schema.
    #[must_use]
    pub fn diff(&self, baseline_schema: &ResolvedTelemetrySchema) -> SchemaChanges {
        let mut changes = SchemaChanges::new();

        if let Some(ref manifest) = self.registry_manifest {
            changes.set_head_manifest(weaver_version::schema_changes::RegistryManifest {
                semconv_version: manifest.semconv_version.clone(),
            });
        }

        if let Some(ref manifest) = baseline_schema.registry_manifest {
            changes.set_baseline_manifest(weaver_version::schema_changes::RegistryManifest {
                semconv_version: manifest.semconv_version.clone(),
            });
        }

        // Attributes in the registry
        self.diff_attributes(baseline_schema, &mut changes);

        // Signals
        let latest_signals = self.groups(GroupType::Metric);
        let baseline_signals = baseline_schema.groups(GroupType::Metric);
        self.diff_signals(
            SchemaItemType::Metrics,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        let latest_signals = self.groups(GroupType::Event);
        let baseline_signals = baseline_schema.groups(GroupType::Event);
        self.diff_signals(
            SchemaItemType::Events,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        let latest_signals = self.groups(GroupType::Span);
        let baseline_signals = baseline_schema.groups(GroupType::Span);
        self.diff_signals(
            SchemaItemType::Spans,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        let latest_signals = self.groups(GroupType::Resource);
        let baseline_signals = baseline_schema.groups(GroupType::Resource);
        self.diff_signals(
            SchemaItemType::Resources,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );

        changes
    }

    fn diff_attributes(
        &self,
        baseline_schema: &ResolvedTelemetrySchema,
        changes: &mut SchemaChanges,
    ) {
        let latest_attributes = self.registry_attribute_map();
        let baseline_attributes = baseline_schema.registry_attribute_map();

        // TODO for future PR, process differences at the field level (not required for the schema update)

        // Collect all the information related to the attributes that have been
        // deprecated in the latest schema.
        for (attr_name, attr) in latest_attributes.iter() {
            let baseline_attr = baseline_attributes.get(attr_name);

            if let Some(baseline_attr) = baseline_attr {
                if let Some(deprecated) = attr.deprecated.as_ref() {
                    // Is this a change from the baseline?
                    if let Some(baseline_deprecated) = baseline_attr.deprecated.as_ref() {
                        if deprecated == baseline_deprecated {
                            // This attribute was already deprecated in the baseline.
                            // We can skip it.
                            continue;
                        }
                    }

                    // It's a new deprecation, detect changes.
                    match deprecated {
                        Deprecated::Updated {
                         ..
                        } => {
                            // TODO(bwplotka): Implement.
                            panic!("not implemented")
                        }
                        Deprecated::Renamed {
                            renamed_to: rename_to,
                            note,
                        } => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Renamed(SchemaItemChangeRenamed{
                                    old_name: baseline_attr.name.clone(),
                                    new_name: rename_to.clone(),
                                    note: note.clone(),
                                }),
                            );
                        }
                        Deprecated::Obsoleted { note } => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Obsoleted {
                                    name: attr.name.clone(),
                                    note: note.clone(),
                                },
                            );
                        }
                        Deprecated::Uncategorized { note } => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Uncategorized {
                                    name: attr.name.clone(),
                                    note: note.clone(),
                                },
                            );
                        }
                    }
                }
            } else {
                changes.add_change(
                    SchemaItemType::RegistryAttributes,
                    SchemaItemChange::Added {
                        name: attr.name.clone(),
                    },
                );
            }
        }

        // Any attribute in the baseline schema that is not present in the latest schema
        // is considered removed.
        // Note: This should never occur if the registry evolution process is followed.
        // However, detecting this case is useful for identifying a violation of the process.
        for (attr_name, attr) in baseline_attributes.iter() {
            if !latest_attributes.contains_key(attr_name) {
                changes.add_change(
                    SchemaItemType::RegistryAttributes,
                    SchemaItemChange::Removed {
                        name: attr.name.clone(),
                    },
                );
            }
        }
    }

    fn diff_signals(
        &self,
        schema_item_type: SchemaItemType,
        latest_signals: &HashMap<&str, &Group>,
        baseline_signals: &HashMap<&str, &Group>,
        changes: &mut SchemaChanges,
    ) {
        // Collect all the information related to the signals that have been
        // deprecated in the latest schema.
        for (id, group) in latest_signals.iter() {
            let baseline_group = baseline_signals.get(id);

            if let Some(baseline_group) = baseline_group {
                if let Some(deprecated) = group.deprecated.as_ref() {
                    // is this a change from the baseline?
                    if let Some(baseline_deprecated) = baseline_group.deprecated.as_ref() {
                        if deprecated == baseline_deprecated {
                            continue;
                        }
                    }

                    match deprecated {
                        Deprecated::Updated(up) => {
                            // TODO(bwplotka): Would it be safer to take baseline content for group.id,
                            // for the replaced item?
                            match self.detect_replace_updated_change(up, group, latest_signals) {
                                Ok(up) => {
                                    changes.add_change(
                                        schema_item_type,
                                        SchemaItemChange::Updated(up),
                                    );
                                }
                                Err(e) => {
                                    changes.add_change(
                                        schema_item_type,
                                        SchemaItemChange::Uncategorized {
                                            name: (*id).to_owned(),
                                            note: e.to_string(),
                                        },
                                    );
                                }
                            }
                        }
                        Deprecated::Renamed {
                            renamed_to: rename_to,
                            note,
                        } => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Renamed(SchemaItemChangeRenamed{
                                    old_name: (*id).to_owned(),
                                    new_name: rename_to.clone(),
                                    note: note.clone(),
                                }),
                            );
                        }
                        Deprecated::Obsoleted { note } => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Obsoleted {
                                    name: (*id).to_owned(),
                                    note: note.clone(),
                                },
                            );
                        }
                        Deprecated::Uncategorized { note } => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Uncategorized {
                                    name: (*id).to_owned(),
                                    note: note.clone(),
                                },
                            );
                        }
                    }
                }
            } else {
                changes.add_change(
                    schema_item_type,
                    SchemaItemChange::Added {
                        name: (*id).to_owned(),
                    },
                );
            }
        }

        // Any signal in the baseline schema that is not present in the latest schema
        // is considered removed.
        // Note: This should never occur if the registry evolution process is followed.
        // However, detecting this case is useful for identifying a violation of the process.
        for (id, _) in baseline_signals.iter() {
            if !latest_signals.contains_key(id) {
                changes.add_change(
                    schema_item_type,
                    SchemaItemChange::Removed {
                        name: (*id).to_owned(),
                    },
                );
            }
        }
    }

    fn detect_replace_updated_change(&self, deprecated_entry: &DeprecatedUpdated, deprecated_group: &Group, latest_signals: &HashMap<&str, &Group>) -> Result<SchemaItemChangeUpdated, String>{
        if !is_simple_mode_enabled() {
            // TODO(bwplotka): Expand globally.
            return Err("generally, this feature implementation requires --simple mode as of now.".to_owned())
        }

        let replacement = match latest_signals.get(&deprecated_entry.replaced_by_id as &str) {
            Some(replacement) => replacement,
            None => return Err(format!("deprecated with the reason update have replaced_by_id {} that can't be found. Can't compile this change; note {}", deprecated_entry.replaced_by_id, deprecated_entry.note)),
        };

        // Fields.
        let mut field_changes: HashMap<String, SchemaItemChangeRenamed> = HashMap::new();
        if replacement.metric_name != deprecated_group.metric_name {
            _ = field_changes.insert("metric_name".to_owned(), SchemaItemChangeRenamed{
                old_name: deprecated_group.metric_name.clone().unwrap_or_else(|| "".to_owned()),
                new_name: replacement.metric_name.clone().unwrap_or_else(|| "".to_owned()),
                note: "".to_owned(),
            });
        }
        if replacement.unit != deprecated_group.unit {
            _ = field_changes.insert("unit".to_owned(), SchemaItemChangeRenamed{
                old_name: deprecated_group.unit.clone().unwrap_or_else(|| "".to_owned()),
                new_name: replacement.unit.clone().unwrap_or_else(|| "".to_owned()),
                note: "".to_owned(),
            });
        }
        if replacement.instrument != deprecated_group.instrument {
            let replacement_instr = match &replacement.instrument {
                Some(s) => s,
                None => return Err(format!("deprecated with the reason update replaced_by_id {} replacement does not have instrument specified; for metrics it's required. Can't compile this change; note {}", deprecated_entry.replaced_by_id, deprecated_entry.note)),
            };
            let deprecated_instr = match &deprecated_group.instrument {
                Some(s) => s,
                None => return Err(format!("deprecated with the reason update does not have instrument specified; for metrics it's required. Can't compile this change; note {}", deprecated_entry.note)),
            };
            _ = field_changes.insert("instrument".to_owned(), SchemaItemChangeRenamed{
                old_name: replacement_instr.to_string(),
                new_name: deprecated_instr.to_string(),
                note: "".to_owned(),
            });
        }

        // Diff local attributes (should it be done in the diff_attributes and injected in the related change?)
        // Make it work for local only attributes for now.
        let mut attribute_changes: HashMap<String, SchemaItemChange> = HashMap::new();
        let deprecated_attributes = Attribute::local_only_map(
            deprecated_group.attributes(&self.catalog).unwrap_or_else(|_| {Vec::new()}));
        let replacement_attributes = Attribute::local_only_map(
            replacement.attributes(&self.catalog).unwrap_or_else(|_| {Vec::new()}));

        for (replacement_attr_local_id, replacement_attr) in &replacement_attributes {
            let deprecated_attr_match = deprecated_attributes.get(replacement_attr_local_id);
            if deprecated_attr_match.is_none() {
                // TODO(bwplotka): Those are not easily transformable for consumers (?) should we
                // mark this deprecation as not tranformable?
                _ = attribute_changes.insert(replacement_attr_local_id.to_string(), SchemaItemChange::Added{
                    name: replacement_attr_local_id.to_string(),
                });
                continue
            }
            // TODO(bwplotka): Handle gaps, we only check tag and members for now.
            let deprecated_attr = deprecated_attr_match.unwrap();
            {
                let d_attr_tag = match &deprecated_attr.tag {
                    Some(s) => s,
                    None => return Err(format!("tag must be specified {:?} Can't compile this change; note {}", deprecated_attr, deprecated_entry.note)),
                };
                let r_attr_tag = match &replacement_attr.tag {
                    Some(s) => s,
                    None => return Err(format!("tag must be specified {:?} Can't compile this change; note {}", replacement_attr, deprecated_entry.note)),
                };
                if d_attr_tag != r_attr_tag {
                    _ = attribute_changes.insert(replacement_attr_local_id.to_string(), SchemaItemChange::Renamed(SchemaItemChangeRenamed{
                        old_name: d_attr_tag.to_string(),
                        new_name: r_attr_tag.to_string(),
                        note: "".to_owned(),
                    }));
                }
            }
            if deprecated_attr.r#type.type_id() != replacement_attr.r#type.type_id() {
                return Err(format!("attribute change type; not supported for now replacement {:?} vs deprecated {:?}. Can't compile this change; note {}", replacement_attr, deprecated_attr, deprecated_entry.note))
            }

            // TODO(bwplotka): This is where I stopped the diff work. I realized even if I
            // manage to find a generic change record that will work for global vs local attribute, metric
            // fields but also members, I might need a totally different structure of "variants".
            // Trying this on a different branch (:
            // match (deprecated_attr, replacement_attr) {
            //     (
            //         AttributeType::Enum { members: members1, .. },
            //         AttributeType::Enum { members: members2, .. },
            //     ) => {
            //        // How to represent them?
            //     },
            //     _ => {},
            // }


            // TODO(bwplotka): It would be useful to do some full validation if no other diff
            // element was missed (which def was, not everything is implemented).
        }
        for (deprecated_attr_local_id, deprecated_attr) in &deprecated_attributes {
            // TODO(bwplotka): Those are not easily transformable for consumers (?) should we
            // mark this deprecation as not convertable?
            if replacement_attributes.get(deprecated_attr_local_id).is_none() {
                _ = attribute_changes.insert(deprecated_attr.name.to_string(), SchemaItemChange::Removed{
                    name: deprecated_attr.name.to_string(),
                });
            }
        }

        Ok(SchemaItemChangeUpdated{
            id: deprecated_group.id.to_owned(),
            replaced_by_id: deprecated_entry.replaced_by_id.clone(),
            forward_promql: deprecated_entry.forward_promql.clone(),
            backward_promql: deprecated_entry.backward_promql.clone(),
            fields: field_changes,
            attributes: attribute_changes,
            note: deprecated_entry.note.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::attribute::Attribute;
    use crate::ResolvedTelemetrySchema;
    use schemars::schema_for;
    use serde_json::to_string_pretty;
    use weaver_semconv::deprecated::Deprecated;
    use weaver_version::schema_changes::{SchemaItemChange, SchemaItemType};

    #[test]
    fn test_json_schema_gen() {
        // Ensure the JSON schema can be generated for the ResolvedTelemetrySchema
        let schema = schema_for!(ResolvedTelemetrySchema);

        // Ensure the schema can be serialized to a string
        assert!(to_string_pretty(&schema).is_ok());
    }

    #[test]
    fn no_diff() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        let changes = prior_schema.diff(&prior_schema);
        assert!(changes.is_empty());
    }

    #[test]
    fn detect_2_added_registry_attributes() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_added_registry_attributes(), 2);
    }

    #[test]
    fn detect_2_deprecated_registry_attributes() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
                Attribute::double("attr5", "brief5", "note5").deprecated(Deprecated::Obsoleted {
                    note: "".to_owned(),
                }),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2")
                    .deprecated(Deprecated::Obsoleted {
                        note: "This attribute is deprecated (deprecated).".to_owned(),
                    })
                    .brief("This attribute is deprecated (brief)."),
                Attribute::int("attr3", "brief3", "note3")
                    .deprecated(Deprecated::Obsoleted {
                        note: "".to_owned(),
                    })
                    .brief("This attribute is deprecated."),
                Attribute::double("attr4", "brief4", "note4"),
                Attribute::double("attr5", "brief5", "note5").deprecated(Deprecated::Obsoleted {
                    note: "".to_owned(),
                }),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_obsoleted_registry_attributes(), 2);
        for attr_change in changes
            .changes_by_type(SchemaItemType::RegistryAttributes)
            .unwrap()
        {
            match attr_change {
                SchemaItemChange::Obsoleted { name, note } => {
                    if name == "attr2" {
                        assert_eq!(note, "This attribute is deprecated (deprecated).");
                    } else if name == "attr3" {
                        assert_eq!(note, "");
                    } else {
                        panic!("Unexpected attribute name.");
                    }
                }
                _ => panic!("Unexpected change type."),
            }
        }
    }

    #[test]
    fn detect_2_renamed_registry_attributes() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        // 2 new attributes are added: attr2_bis and attr3_bis
        // attr2 is renamed attr2_bis
        // attr3 is renamed attr3_bis
        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2").deprecated(Deprecated::Renamed {
                    renamed_to: "attr2_bis".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr3_bis".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );
        latest_schema.add_attribute_group(
            "registry.group2",
            [
                Attribute::boolean("attr2_bis", "brief1", "note1"),
                Attribute::boolean("attr3_bis", "brief1", "note1"),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        dbg!(&changes);
        assert_eq!(changes.count_changes(), 4);
        assert_eq!(changes.count_registry_attribute_changes(), 4);
        assert_eq!(changes.count_renamed_registry_attributes(), 2);
        assert_eq!(changes.count_added_registry_attributes(), 2);
    }

    #[test]
    fn detect_2_attributes_renamed_to_the_same_existing_attribute() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::string("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );
        prior_schema.add_attribute_group("group2", [Attribute::string("attr5", "brief", "note")]);

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );
        latest_schema.add_attribute_group("group2", [Attribute::string("attr5", "brief", "note")]);

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_renamed_registry_attributes(), 2);
        dbg!(&changes);
    }

    #[test]
    fn detect_2_attributes_renamed_to_the_same_new_attribute() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::string("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );
        latest_schema.add_attribute_group(
            "registry.group2",
            [Attribute::string("attr5", "brief", "note")],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 3);
        assert_eq!(changes.count_registry_attribute_changes(), 3);
        assert_eq!(changes.count_renamed_registry_attributes(), 2);
        assert_eq!(changes.count_added_registry_attributes(), 1);
        dbg!(&changes);
    }

    /// In normal situation this should never happen based on the registry evolution process.
    /// However, detecting this case is useful for identifying a violation of the process.
    #[test]
    fn detect_2_removed_attributes() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_removed_registry_attributes(), 2);
    }
}
