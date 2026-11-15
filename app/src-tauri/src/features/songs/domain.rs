// Una letra de la biblioteca. El cuerpo lleva las diapositivas separadas por una línea con «****».
use serde::{Deserialize, Serialize};

use crate::features::output::domain::TextAnimation;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub title: String,
    pub singer: String,
    pub writer: String,
    pub body: String,
    pub background: Option<String>,
    /// Su propia animación (D-024); None = la del plan o la general. «Sin animación» es una con todo en ninguno.
    #[serde(default)]
    pub animation: Option<TextAnimation>,
}
