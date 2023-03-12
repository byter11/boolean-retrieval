use std::cmp::Ordering;
use std::collections::{HashMap, LinkedList};
use std::fmt::{self, Debug};
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use rust_stemmers::{Algorithm, Stemmer};
use serde::{Deserialize, Serialize};

const STOPWORDS: &[&str] = &[
    "a", "is", "the", "of ", "all", "and ", "to", "can", "be", "as", "once ", "for", "at", "am",
    "are", "has", "have", "had", "up", "his", "her", "in", "on", "no", "we", "do",
];

/// Data structure definition for a document in the boolean model
#[derive(Eq, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    pub id: u32,
    positions: LinkedList<u32>,
}

impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        return self.id == other.id;
    }
}

impl Hash for Document {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return f
            .debug_struct("Document")
            .field("id", &self.id)
            .field("positions", &self.positions)
            .finish();
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct DocumentDetails {
    pub name: String,
    pub summary: String,
    pub text: String,
}

fn stemmer_default() -> Stemmer {
    Stemmer::create(Algorithm::English)
}

/// Boolean Query Operators
enum Op {
    AND,
    OR,
    NONE,
}

#[derive(Serialize, Deserialize)]
pub struct BooleanModel {
    posting_list: HashMap<String, Vec<Document>>,
    documents: HashMap<u32, DocumentDetails>,

    #[serde(skip, default = "stemmer_default")]
    stemmer: Stemmer,
}

impl Default for BooleanModel {
    fn default() -> Self {
        return BooleanModel::new();
    }
}

impl BooleanModel {
    pub fn new() -> Self {
        Self {
            posting_list: HashMap::new(),
            documents: HashMap::new(),
            stemmer: stemmer_default(),
        }
    }

    /// This public function takes in a directory containing text files only
    /// Creates a posting list by:
    /// 1. Filtering text (removing non alphanumeric symbols)
    /// 2. Tokenizing by splitting on space
    /// 3. Removing stopwords from tokens
    /// 4. Stemming tokens
    /// 5. Adding Documents to the posting list with positions
    pub fn index(self: &mut Self, data_dir: PathBuf) {
        let files = BooleanModel::list_dir_sorted(&data_dir);

        for (i, file) in files.iter().enumerate() {
            let doc_id = i as u32 + 1;
            let text = fs::read_to_string(file);
            if text.is_err() {
                println!("error reading {:?}: {:?}", file, text.err().unwrap());
                continue;
            }
            let text = text.unwrap();

            // TODO: fix cases such as "don't" becoming 2 tokens "don" and "t". use .map instead
            let filtered_text = text
                .to_lowercase()
                .replace(|c: char| !c.is_ascii_alphanumeric(), " ");

            BooleanModel::tokenize(&filtered_text)
                .filter(|t| !STOPWORDS.contains(t))
                .into_iter()
                .enumerate()
                .for_each(|(j, token)| {
                    self.insert(
                        &self.stem(token),
                        Document {
                            id: doc_id,
                            positions: LinkedList::from([j as u32]),
                        },
                    )
                });

            self.documents.insert(
                doc_id,
                DocumentDetails {
                    name: String::from(file.file_name().unwrap().to_str().unwrap()),
                    summary: String::from(filtered_text.get(0..50).unwrap_or_default()),
                    text,
                },
            );
        }
    }

    /// Takes in a `query` of the form "X AND Y OR Z ..."
    /// Returns vector of `Document`s by applying intersection or union to document lists
    pub fn query_boolean(self: &Self, query: &str) -> Vec<Document> {
        let mut op = Op::NONE;

        let docs =
            BooleanModel::tokenize(&String::from(query)).fold(vec![], |ans, token| match token {
                "AND" => {
                    op = Op::AND;
                    ans
                }
                "OR" => {
                    op = Op::OR;
                    ans
                }
                other => {
                    let docs = self
                        .get_docs(other.to_lowercase().as_str())
                        .unwrap_or(vec![]);

                    match op {
                        Op::AND => {
                            return BooleanModel::intersect(&ans, &docs);
                        }
                        Op::OR => {
                            return BooleanModel::union(&ans, &docs);
                        }
                        Op::NONE => {
                            if ans.is_empty() {
                                return docs;
                            }
                        }
                    }
                    op = Op::NONE;
                    ans
                }
            });

        docs.into_iter().map(|f| f.clone()).collect()
    }

    /// Takes in a `query` of the form "X Y Z ... /k"
    /// Returns vector of `Document`s by intersecting based on term proximity in the document
    pub fn query_positional(self: &Self, query: &str) -> Vec<Document> {
        let mut docs_list = vec![];
        let mut k = 1;

        for token in BooleanModel::tokenize(&String::from(query)) {
            match token {
                pos if pos.starts_with("/") => match pos[1..].parse() {
                    Ok(tk) => k = tk,
                    Err(_) => k = 1,
                },
                term => {
                    docs_list.push(
                        self.get_docs(term.to_lowercase().as_str())
                            .unwrap_or(vec![]),
                    );
                }
            }
        }

        let docs = docs_list
            .into_iter()
            .enumerate()
            .fold(vec![], |ans, (i, cur)| {
                if i == 0 {
                    return cur;
                }
                BooleanModel::positional_intersect(&ans, &cur, k)
            });

        docs.into_iter().map(|f| f.clone()).collect()
    }

    pub fn get_doc(self: &Self, id: u32) -> Option<&DocumentDetails> {
        return self.documents.get(&id);
    }

    /// Return new vector containing references of `Document`s containing a `term`
    fn get_docs(self: &Self, term: &str) -> Option<Vec<&Document>> {
        self.posting_list
            .get(&self.stem(term))
            .and_then(|list| Some(list.iter().collect()))
    }

    fn union<'a>(a: &Vec<&'a Document>, b: &Vec<&'a Document>) -> Vec<&'a Document> {
        let mut result = vec![];

        let mut i = 0;
        let mut j = 0;

        while i < a.len() && j < b.len() {
            if a[i].id == b[j].id {
                result.push(b[j]);
                i = i + 1;
                j = j + 1;
            } else if a[i].id < b[j].id {
                result.push(a[i]);
                i = i + 1;
            } else {
                result.push(b[j]);
                j = j + 1;
            }
        }

        while i < a.len() {
            result.push(a[i]);
            i = i + 1;
        }

        while j < b.len() {
            result.push(a[j]);
            j = j + 1;
        }

        result
    }

    fn intersect<'a>(a: &Vec<&'a Document>, b: &Vec<&'a Document>) -> Vec<&'a Document> {
        let mut result = vec![];

        let mut i = 0;
        let mut j = 0;

        while i < a.len() && j < b.len() {
            if a[i].id == b[j].id {
                result.push(b[j]);
                i = i + 1;
                j = j + 1;
            } else if a[i].id < b[j].id {
                i = i + 1;
            } else {
                j = j + 1;
            }
        }

        result
    }

    fn positional_intersect<'a>(
        a: &Vec<&'a Document>,
        b: &Vec<&'a Document>,
        k: u32,
    ) -> Vec<&'a Document> {
        let mut answer = vec![];

        let mut i = 0;
        let mut j = 0;

        while i < a.len() && j < b.len() {
            if a[i].id == b[j].id {
                let mut ok = false;

                for pp1 in &a[i].positions {
                    if ok {
                        answer.push(b[j]);
                        break;
                    }

                    for pp2 in &b[j].positions {
                        if pp1.abs_diff(*pp2) <= k {
                            ok = true;
                        } else if pp2 > pp1 {
                            break;
                        }
                    }
                }
                i = i + 1;
                j = j + 1;
            } else if a[i].id < b[j].id {
                i = i + 1;
            } else {
                j = j + 1;
            }
        }

        answer
    }

    /// Adds document to posting list based on these criterias:
    /// 1. if posting list contains `term` and the `document`: append to `positions`
    /// 2. if posting list contains `term`: insert document`
    /// 3. else insert new vector with the document to `posting_list[term]`
    fn insert(&mut self, term: &str, mut document: Document) {
        if self.posting_list.contains_key(term) {
            let idx_result =
                self.posting_list[term].binary_search_by(|doc| doc.id.cmp(&document.id));
            match idx_result {
                Ok(idx) => {
                    self.posting_list.get_mut(term).unwrap()[idx]
                        .positions
                        .append(&mut document.positions);
                }
                Err(_) => self.posting_list.get_mut(term).unwrap().push(document),
            }
        } else {
            self.posting_list.insert(term.to_string(), vec![document]);
        }
    }

    /// 1. Split on ' '
    /// 2. Remove words with length <= 1
    fn tokenize(s: &String) -> impl Iterator<Item = &str> {
        s.split(' ').filter(|&x| x.len() > 1)
    }

    /// Stem a string using the porter stemmer algorithm
    fn stem(self: &Self, s: &str) -> String {
        self.stemmer.stem(s).to_string()
    }

    /// Return all files in a directory as a list
    fn list_dir_sorted(path: &Path) -> Vec<PathBuf> {
        let mut files = fs::read_dir(path)
            .unwrap()
            .map(|x| x.unwrap().path())
            .collect::<Vec<PathBuf>>();

        files.sort_by(
            |a, b| match a.to_str().unwrap().len().cmp(&b.to_str().unwrap().len()) {
                Ordering::Equal => a.cmp(&b),
                other => other,
            },
        );

        files
    }
}

impl Debug for BooleanModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.posting_list.fmt(f)
    }
}
