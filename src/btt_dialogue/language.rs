use std::error::Error;

use ironworks::{
    Ironworks,
    excel::Language,
    file::{File, exh::ExcelHeader},
};

// BTT language table
// --------------------------------
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BttLanguage {
    code: &'static str,
    client_file_code: &'static str,
    ironworks: Language,
}

impl BttLanguage {
    pub(super) fn code(self) -> &'static str {
        self.code
    }

    pub(super) fn client_file_code(self) -> &'static str {
        self.client_file_code
    }

    pub(super) fn ironworks(self) -> Language {
        self.ironworks
    }
}

pub(super) const ALL_LANGUAGES: [BttLanguage; 7] = [
    BttLanguage {
        code: "ja",
        client_file_code: "ja",
        ironworks: Language::Japanese,
    },
    BttLanguage {
        code: "en",
        client_file_code: "en",
        ironworks: Language::English,
    },
    BttLanguage {
        code: "de",
        client_file_code: "de",
        ironworks: Language::German,
    },
    BttLanguage {
        code: "fr",
        client_file_code: "fr",
        ironworks: Language::French,
    },
    BttLanguage {
        code: "zh-Hans",
        client_file_code: "chs",
        ironworks: Language::ChineseSimplified,
    },
    BttLanguage {
        code: "zh-Hant",
        client_file_code: "tc",
        ironworks: Language::ChineseTraditional,
    },
    BttLanguage {
        code: "ko",
        client_file_code: "ko",
        ironworks: Language::Korean,
    },
];

pub(super) fn from_code(code: &str) -> Option<BttLanguage> {
    ALL_LANGUAGES
        .iter()
        .copied()
        .find(|language| language.code == code)
}

// export language selection
// --------------------------------
pub(super) fn select_export_languages(
    ironworks: &Ironworks,
    requested: Option<&[String]>,
) -> Result<Vec<BttLanguage>, Box<dyn Error>> {
    let available = available_languages(ironworks)?;
    if let Some(requested) = requested {
        let mut selected = Vec::new();
        for code in requested {
            let code = code.as_str();
            if selected
                .iter()
                .any(|language: &BttLanguage| language.code() == code)
            {
                return Err(format!("Duplicate language code: {code}").into());
            }

            let language = from_code(code)
                .ok_or_else(|| format!("Unsupported BTT dialogue language code: {code}"))?;
            if !available.contains(&language) {
                return Err(format!("Language is not available in this client: {code}").into());
            }

            selected.push(language);
        }

        return Ok(selected);
    }

    Ok(available)
}

// client availability probe
// --------------------------------
pub(super) fn available_languages(
    ironworks: &Ironworks,
) -> Result<Vec<BttLanguage>, Box<dyn Error>> {
    let header = ironworks.file::<ExcelHeader>("exd/Item.exh")?;
    let declared = header
        .languages
        .into_iter()
        .map(Language::from)
        .collect::<Vec<_>>();

    Ok(ALL_LANGUAGES
        .iter()
        .copied()
        .filter(|language| declared.contains(&language.ironworks))
        // Regional client support is proven by the actual EXD file, not only
        // by Ironworks being able to name the language.
        .filter(|language| {
            ironworks
                .file::<FileExists>(&format!("exd/Item_0_{}.exd", language.client_file_code()))
                .is_ok()
        })
        .collect())
}

struct FileExists;

impl File for FileExists {
    fn read(_stream: impl ironworks::FileStream) -> Result<Self, ironworks::Error> {
        Ok(Self)
    }
}
