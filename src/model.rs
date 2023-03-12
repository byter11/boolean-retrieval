use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, LinkedList};
use std::fmt::{self, Debug};
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use serde::ser::SerializeStruct;
use serde::Serialize;

#[derive(Eq, Clone, Default)]
pub struct Document {
    id: u32,
    pub name: String,
    positions: LinkedList<u32>,
    pub summary: String,
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

impl Serialize for Document {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Document", 2)?;
        state.serialize_field("name", &self.name).ok();
        state.serialize_field("summary", &self.summary).ok();
        state.end()
    }
}
impl Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return f
            .debug_struct("Document")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("positions", &self.positions)
            .finish();
    }
}

enum Op {
    AND,
    OR,
    NONE,
}

pub struct BooleanModel {
    posting_list: HashMap<String, Vec<Document>>,
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
        }
    }

    pub fn index(self: &mut Self, data_dir: PathBuf, stopwords_file: PathBuf) {
        let files = BooleanModel::list_dir_sorted(&data_dir);
        let stopwords_text = fs::read_to_string(stopwords_file).unwrap_or_default();
        let stopwords = stopwords_text
            .split('\n')
            .filter(|x| !x.is_empty())
            .collect::<HashSet<&str>>();

        for (i, file) in files.iter().enumerate() {
            let doc_id = i as u32 + 1;
            let text = fs::read_to_string(file);

            let filtered_text = match text {
                // TODO: fix cases such as "don't" becoming 2 tokens "don" and "t". use .map instead
                Ok(text) => text
                    .to_lowercase()
                    .replace(|c: char| !c.is_ascii_alphanumeric(), " "),
                Err(err) => {
                    dbg!("error reading {:?}: {:?}", file, err);
                    String::new()
                }
            };

            BooleanModel::tokenize(&filtered_text)
                .filter(|t| !stopwords.contains(t))
                .map(BooleanModel::stem)
                .into_iter()
                .enumerate()
                .for_each(|(j, token)| {
                    self.insert(
                        token,
                        Document {
                            id: doc_id,
                            name: String::from(file.file_name().unwrap().to_str().unwrap()),
                            summary: String::from(&filtered_text[0..50]),
                            positions: LinkedList::from([j as u32]),
                        },
                    )
                });
        }
    }

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
                    let docs = self.get_docs(other.to_lowercase().as_str()).unwrap_or(vec![]);

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

        return docs.into_iter().map(|f| f.clone()).collect();
    }

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
                    docs_list.push(self.get_docs(term.to_lowercase().as_str()).unwrap_or(vec![]));
                }
            }
        }

        let docs = docs_list.into_iter().enumerate().fold(vec![], |ans, (i, cur)| {
            if i == 0 {
                return cur;
            }
            BooleanModel::positional_intersect(&ans, &cur, k)
        });

        dbg!(query, k, &docs);

        return docs.into_iter().map(|f| f.clone()).collect();
    }

    fn get_docs(self: &Self, term: &str) -> Option<Vec<&Document>> {
        self.posting_list
            .get(term)
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

                dbg!(&a[i].positions);
                for pp1 in &a[i].positions {
                    if ok {
                        answer.push(b[j]);
                        break;
                    }

                    dbg!(&b[j].positions);
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
                dbg!(ok);
            } else if a[i].id < b[j].id {
                i = i + 1;
            } else {
                j = j + 1;
            }
        }

        return answer;
    }

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

    fn tokenize(s: &String) -> impl Iterator<Item = &str> {
        s.split(' ').filter(|&x| x.len() > 1)
    }

    fn stem(s: &str) -> &str {
        s
    }

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
        return self.posting_list.fmt(f);
    }
}
