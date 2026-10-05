//! Prompt engineering and taxonomy helpers for SPAI 5D facet classification.
use crate::slices::spai_notes::discovery::SpaiProjectSummary;
use crate::slices::spai_notes::note::SpaiNoteItem;
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Taxonomy {
    pub tags: Vec<String>,
    pub projects: Vec<String>,
    pub areas: Vec<String>,
}

/// Collects all unique existing tags, projects, and area facets across discovered projects.
pub fn collect_taxonomy(projects: &[SpaiProjectSummary]) -> Taxonomy {
    let mut tags = HashSet::new();
    let mut projs = HashSet::new();
    let mut areas = HashSet::new();

    for p in projects {
        projs.insert(p.name.clone());
        for item in &p.items {
            for tag in &item.tags {
                tags.insert(tag.clone());
            }
            if let Some(proj) = &item.facets.project {
                projs.insert(proj.clone());
            }
            if let Some(area) = &item.facets.area {
                areas.insert(area.clone());
            }
        }
    }

    let mut tag_list: Vec<String> = tags.into_iter().collect();
    tag_list.sort();
    let mut proj_list: Vec<String> = projs.into_iter().collect();
    proj_list.sort();
    let mut area_list: Vec<String> = areas.into_iter().collect();
    area_list.sort();

    Taxonomy {
        tags: tag_list,
        projects: proj_list,
        areas: area_list,
    }
}

pub const SYSTEM_PROMPT: &str =
    "Jsi asistent pro správu osobních znalostí, úkolů a poznámek SPAI. Odpovídáš výhradně ve formátu JSON.";

pub fn build_classification_prompt(item: &SpaiNoteItem, taxonomy: Option<&Taxonomy>) -> String {
    let taxonomy_section = if let Some(tax) = taxonomy {
        let tags_str = if tax.tags.is_empty() { "žádné".to_string() } else { tax.tags.join(", ") };
        let projs_str = if tax.projects.is_empty() { "žádné".to_string() } else { tax.projects.join(", ") };
        let areas_str = if tax.areas.is_empty() { "žádné".to_string() } else { tax.areas.join(", ") };

        format!(
            "\nDŮLEŽITÉ PRO TAXONOMICKOU KONZISTENCI (mozek):\n\
Zde je seznam již existujících štítků (tags), projektů (project) a oblastí (area) v systému uživatele.\n\
Při generování metadat prioritně vybírej z těchto existujících seznamů.\n\
Nové štítky, projekty nebo oblasti vymysli a přidej pouze v případě, že žádný z existujících neodpovídá obsahu.\n\
\n\
Existující štítky (tags): {}\n\
Existující projekty (project): {}\n\
Existující oblasti (area): {}\n",
            tags_str, projs_str, areas_str
        )
    } else {
        String::new()
    };

    format!(
        "Analyzuj následující text poznámky/úkolu a vygeneruj pro ni strukturovaná 5D metadata ve formátu JSON.\n\
Obsah záznamu:\n\
\"\"\"\n\
Titulek: {}\n\
Typ: {}\n\
Stav: {}\n\
Tagy: {}\n\
Datum: {}\n\
Tělo:\n\
{}\n\
\"\"\"\n\
{}\n\
Vrať pouze čistý JSON objekt s následující strukturou:\n\
{{\n\
  \"title\": \"Stručný výstižný název poznámky\",\n\
  \"description\": \"Jednověté shrnutí obsahu\",\n\
  \"tags\": [\"tag1\", \"tag2\"],\n\
  \"facets\": {{\n\
    \"area\": \"oblast/kategorie (např. Frontend, Backend, Architecture, DevOps, Database, Docs, Ops, Notes, Tasks), nebo null\",\n\
    \"effort\": \"odhad náročnosti: low, medium, high, nebo null\",\n\
    \"urgency\": \"naléhavost: low, medium, high, critical, nebo null\",\n\
    \"who\": \"role/řešitel (např. Lead, Developer, Agent, User, External), nebo null\",\n\
    \"project\": \"název projektu, ke kterému záznam patří, nebo null\",\n\
    \"priority\": \"high, medium, low nebo null\",\n\
    \"deadline\": \"YYYY-MM-DD nebo YYYY-MM-DDTHH:mm, nebo null\"\n\
  }}\n\
}}",
        item.title,
        item.kind.as_str(),
        item.status.as_str(),
        item.tags.join(", "),
        item.timestamp,
        item.body,
        taxonomy_section
    )
}
