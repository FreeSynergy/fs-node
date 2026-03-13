// Module plugin manifest — re-exported from fsy-plugin-sdk.
//
// All plugin protocol types live in fsy-plugin-sdk; this module provides
// a stable import path for the rest of fsn-*.

/// Re-export all plugin protocol types from `fsy-plugin-sdk`.
pub use fsy_plugin_sdk::{
    ModuleManifest,
    ManifestInputs,
    ManifestOutputFile,
    PluginContext,
    InstanceInfo,
    PeerService,
    PeerRoute,
    PluginResponse,
    OutputFile,
    ShellCommand,
    LogLine,
    LogLevel,
};
