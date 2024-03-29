//! Data structure and function for working with data collected by the boat.

use std::{
    fmt::Display,
    io::{ErrorKind, Write},
    path::PathBuf,
    str::FromStr,
};

use chrono::{DateTime, Utc};
use geo_types::Point;
use geojson::{
    de::deserialize_geometry, ser::serialize_geometry, FeatureCollection, GeoJson, JsonObject,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map};
use tauri::{
    api::{self, file},
    AppHandle,
};

#[derive(Debug)]
/// Data received from the boat in GeoJSON format.
///
/// # Fields
///
/// `version`: The version of the BoatData format.
/// `features`: The data collected by the boat.
pub struct BoatData {
    version: String,
    features: Vec<BoatDataFeature>,
}

impl Default for BoatData {
    /// Default `BoatData`.
    ///
    /// The version would default to "0.1.0" and an empty feature array.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use babara_project_desktop::data::BoatData;
    ///
    /// let default = BoatData::default();
    /// assert_eq!(default.version, String::from("0.1.0"));
    /// assert_eq!(default.features, vec![]);
    /// ```
    fn default() -> Self {
        Self {
            version: String::from("0.1.0"),
            features: vec![],
        }
    }
}

impl FromStr for BoatData {
    type Err = String;

    /// Creates a new `BoatData` from a GeoJSON string.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let geojson: GeoJson = value.parse().map_err(|e| format!("{e}"))?;

        log::info!("Parsing Feature Collection");
        let features =
            FeatureCollection::try_from(geojson).map_err(|_| String::from("Invalid GeoJSON"))?;
        log::debug!("Feature Collection: {}", features);

        // Checking for version
        log::info!("Checking Version");
        let foreign_members = features
            .foreign_members
            .ok_or(String::from("Invalid Boat Data GeoJSON: Missing Version"))?;
        let version = foreign_members
            .get("version")
            .ok_or(String::from("Invalid Boat Data GeoJSON: Missing Version"))?
            .as_str()
            .ok_or(String::from("Invalid Boat Data GeoJSON: Invalid Version"))?;
        log::debug!("Version: {}", version);

        log::info!("Extracting Features");
        let features = features.features;
        let features = if features.is_empty() {
            vec![]
        } else {
            geojson::de::deserialize_feature_collection_str_to_vec(value)
                .map_err(|_| "Invalid Boat Data GeoJSON: Invalid Data Features")?
        };

        Ok(Self {
            version: String::from(version),
            features,
        })
    }
}

impl Display for BoatData {
    /// Display the `BoatData` in GeoJSON fromat.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", GeoJson::from(self))
    }
}

impl TryFrom<GeoJson> for BoatData {
    type Error = String;

    /// Creates a new `BoatData` from a `GeoJson` struct.
    fn try_from(value: GeoJson) -> Result<Self, Self::Error> {
        value.to_string().parse()
    }
}

impl From<BoatData> for GeoJson {
    /// Converts `BoatData` to `GeoJson` struct.
    fn from(value: BoatData) -> Self {
        GeoJson::from(&value)
    }
}

impl From<&mut BoatData> for GeoJson {
    /// Converts `BoatData` to `GeoJson` struct.
    fn from(value: &mut BoatData) -> Self {
        GeoJson::from(&*value)
    }
}

impl From<&BoatData> for GeoJson {
    /// Converts `BoatData` to `GeoJson` struct.
    fn from(value: &BoatData) -> Self {
        let features = value.features.iter().map(geojson::Feature::from).collect();
        let mut foreign_members = Map::new();
        foreign_members.insert(String::from("version"), json!(&value.version));

        let collection = FeatureCollection {
            bbox: None,
            features,
            foreign_members: Some(foreign_members),
        };
        GeoJson::from(collection)
    }
}

impl Serialize for BoatData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        GeoJson::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BoatData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        GeoJson::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
/// The layer of the water body the data is collected from.
///
/// # Variants
///
/// `Surface`: The data is collected from the surface of the water body.
/// `Middle`: The data is collected from the middle of the water body.
/// `SeaBed`: The data is collected from the sea bed of the water body.
pub enum Layer {
    #[serde(rename = "surface")]
    /// The data is collected from the surface of the water body.
    Surface,
    #[serde(rename = "middle")]
    /// The data is collected from the middle of the water body.
    Middle,
    #[serde(rename = "sea bed")]
    /// The data is collected from the sea bed of the water body.
    SeaBed,
}

impl Display for Layer {
    /// Displays the current layer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use babara_project_desktop::data::Layer;
    ///
    /// let surface = Layer::Surface;
    /// let middle = Layer::Middle;
    /// let seabed = Layer::SeaBed;
    ///
    /// assert_eq!(surface.to_string(), "surface");
    /// assert_eq!(middle.to_string(), "middle");
    /// assert_eq!(seabed.to_string(), "sea bed");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let output = match self {
            Self::Surface => "surface",
            Layer::Middle => "middle",
            Layer::SeaBed => "sea bed",
        };
        write!(f, "{output}")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Individual temperature data received from the boat in GeoJSON format.
///
/// # Fields
///
/// `temperature`: The temperature measured.
/// `depth`: The depth the temperature is collected at.
/// `layer`: The layer of the water body the temperature is collected at.
/// `time`: The date and time the temperature is collected.
/// `geometry`: The coordinate the temperature is collected.
pub struct BoatDataFeature {
    temperature: f64,
    depth: f64,
    layer: Layer,
    time: DateTime<Utc>,
    #[serde(
        serialize_with = "serialize_geometry",
        deserialize_with = "deserialize_geometry"
    )]
    geometry: Point<f64>,
}

impl From<BoatDataFeatureCSV> for BoatDataFeature {
    /// Converts to the CSV representation of the data.
    fn from(value: BoatDataFeatureCSV) -> Self {
        Self::from(&value)
    }
}

impl From<&mut BoatDataFeatureCSV> for BoatDataFeature {
    /// Converts to the CSV representation of the data.
    fn from(value: &mut BoatDataFeatureCSV) -> Self {
        Self::from(&*value)
    }
}

impl From<&BoatDataFeatureCSV> for BoatDataFeature {
    /// Converts to the CSV representation of the data.
    fn from(value: &BoatDataFeatureCSV) -> Self {
        Self {
            geometry: Point::new(value.lng, value.lat),
            time: value.time,
            temperature: value.temperature,
            depth: value.depth,
            layer: value.layer,
        }
    }
}

impl From<BoatDataFeature> for geojson::Feature {
    /// Converts to the `geojson::Feature` struct.
    fn from(value: BoatDataFeature) -> Self {
        Self::from(&value)
    }
}

impl From<&mut BoatDataFeature> for geojson::Feature {
    /// Converts to the `geojson::Feature` struct.
    fn from(value: &mut BoatDataFeature) -> Self {
        Self::from(&*value)
    }
}

impl From<&BoatDataFeature> for geojson::Feature {
    /// Converts to the `geojson::Feature` struct.
    fn from(value: &BoatDataFeature) -> Self {
        let geometry = geojson::Value::from(&value.geometry);

        let mut properties = Map::new();
        properties.insert(String::from("temperature"), value.temperature.into());
        properties.insert(String::from("depth"), value.depth.into());
        properties.insert(String::from("layer"), value.layer.to_string().into());
        properties.insert(String::from("time"), value.time.to_rfc3339().into());

        Self {
            bbox: None,
            geometry: Some(geometry.into()),
            id: None,
            properties: Some(JsonObject::from(properties)),
            foreign_members: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Individual temperature data received from the boat in GeoJSON format.
///
/// # Fields
///
/// `temperature`: The temperature measured.
/// `depth`: The depth the temperature is collected at.
/// `layer`: The layer of the water body the temperature is collected at.
/// `time`: The date and time the temperature is collected.
/// `lat`: The latitude of the coordinate the temperature is collected.
/// `lng`: The longitude of the coordinate the temperature is collected.
pub struct BoatDataFeatureCSV {
    temperature: f64,
    depth: f64,
    layer: Layer,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    time: DateTime<Utc>,
    lat: f64,
    lng: f64,
}

impl From<BoatDataFeature> for BoatDataFeatureCSV {
    /// Converts to the GeoJSON Feature representation of the data.
    fn from(value: BoatDataFeature) -> Self {
        Self::from(&value)
    }
}

impl From<&mut BoatDataFeature> for BoatDataFeatureCSV {
    /// Converts to the GeoJSON Feature representation of the data.
    fn from(value: &mut BoatDataFeature) -> Self {
        Self::from(&*value)
    }
}

impl From<&BoatDataFeature> for BoatDataFeatureCSV {
    /// Converts to the GeoJSON Feature representation of the data.
    fn from(value: &BoatDataFeature) -> Self {
        Self {
            lat: value.geometry.y(),
            lng: value.geometry.x(),
            time: value.time,
            temperature: value.temperature,
            depth: value.depth,
            layer: value.layer,
        }
    }
}

impl From<BoatDataFeatureCSV> for geojson::Feature {
    /// Converts to the `geojson::Feature` struct.
    fn from(value: BoatDataFeatureCSV) -> Self {
        Self::from(&value)
    }
}

impl From<&mut BoatDataFeatureCSV> for geojson::Feature {
    /// Converts to the `geojson::Feature` struct.
    fn from(value: &mut BoatDataFeatureCSV) -> Self {
        Self::from(&*value)
    }
}

impl From<&BoatDataFeatureCSV> for geojson::Feature {
    /// Converts to the `geojson::Feature` struct.
    fn from(value: &BoatDataFeatureCSV) -> Self {
        Self::from(BoatDataFeature::from(value))
    }
}

#[tauri::command]
/// Read boat data from application storage.
pub fn read_data(app_handle: AppHandle) -> Result<BoatData, String> {
    log::debug!("Reading Path");
    let mut data_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .ok_or(String::from("Unable to Get App Data Directory"))?;
    data_dir.push("data.geojson");
    log::debug!("Application GeoJSON Boat Data: {}", data_dir.display());

    import_data(data_dir)
}

#[tauri::command]
/// Import boat data from the file system.
pub fn import_data(import_path: PathBuf) -> Result<BoatData, String> {
    log::debug!("Importing from: {}", import_path.display());
    Ok(match file::read_string(&import_path) {
        Ok(v) => BoatData::from_str(&v)?,
        Err(api::Error::Io(e)) => match e.kind() {
            ErrorKind::NotFound => {
                log::warn!(
                    "Unable to find Path: {}, using default BoatData",
                    import_path.display()
                );
                BoatData::default()
            }
            _ => return Err(e.to_string()),
        },
        Err(e) => return Err(e.to_string()),
    })
}

#[tauri::command]
/// Export boat data to the file system.
pub fn export_data(export_path: PathBuf, data: BoatData) -> Result<(), String> {
    log::debug!("Exporting to: {}", export_path.display());
    let mut file = std::fs::File::create(export_path).map_err(|e| e.to_string())?;
    write!(file, "{}", data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
/// Save boat data to application storage.
pub fn save_data(app_handle: AppHandle, data: BoatData) -> Result<(), String> {
    log::debug!("Saving Path");
    let mut data_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .ok_or(String::from("Unable to Get App Data Directory"))?;
    data_dir.push("data.geojson");
    log::debug!("Application GeoJSON Path: {}", data_dir.display());

    export_data(data_dir, data)
}

#[tauri::command]
/// Export boat data in CSV format to the file system.
pub fn export_data_csv(export_path: PathBuf, data: BoatData) -> Result<(), String> {
    log::debug!("Exporting to: {}", export_path.display());
    let mut writer = csv::Writer::from_path(export_path).map_err(|e| e.to_string())?;
    for record in data.features {
        let record = BoatDataFeatureCSV::from(record);
        writer.serialize(record).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
/// Import boat data in CSV format from the file system.
pub fn import_data_csv(import_path: PathBuf) -> Result<BoatData, String> {
    log::debug!("Importing from: {}", import_path.display());
    Ok(match file::read_string(&import_path) {
        Ok(v) => BoatData {
            version: String::from("0.1.0"),
            features: csv::Reader::from_reader(v.as_bytes())
                .deserialize::<BoatDataFeatureCSV>()
                .map(|v| v.map(BoatDataFeature::from))
                .collect::<Result<Vec<_>, csv::Error>>()
                .map_err(|e| e.to_string())?,
        },
        Err(api::Error::Io(e)) => match e.kind() {
            ErrorKind::NotFound => {
                log::warn!(
                    "Unable to find Path: {}, using default BoatData",
                    import_path.display()
                );
                BoatData::default()
            }
            _ => return Err(e.to_string()),
        },
        Err(e) => return Err(e.to_string()),
    })
}
