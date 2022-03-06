use std::{future::Future, collections::HashMap};
use futures::future::join_all;

use cynic::{QueryBuilder, MutationBuilder, Operation, serde_json::Value};
use queries::ResponseStatus;
use reqwest::{ClientBuilder, header, Response};
use itertools::{Itertools, process_results};
use anyhow::{Result, bail};
use sha1::{Digest, Sha1};
use base64::encode_config_buf;

/// Code for Queries generated using <https://generator.cynic-rs.dev/>. Quirks:
/// 
/// Just paste the schema.graphql content in. It had to be modified to be more
/// compliant. 
/// * Concatenate all the `.graphql` schema files from wiki-js `server/graph/schema`
///   NB: For ease of manipulation, make sure `common.schema` is at the top.
/// * You need to remove any `extends Query|Mutation|Subscription { ... }`
///   and add the content of the extend to `Query`, `Mutation` or `Subscription`.
/// * Remove any `@...` annotations and the `directive @auth...` definition.
/// 
/// These are safe schema transformations, as the @ annotations have meaning only 
/// for the server, and inlining the `extends` is a no-op semantically. 
/// 
/// Note it has some finnicky behaviour. It will generate slightly wrong code. 
/// You may need to make the following substitutions:
/// 
/// * `cynic::FragmentArguments` -> `cynic::QueryVariables`
/// * `#[arguments(name: $name)]` -> `#[arguments(name = &args.name)]`


#[cynic::schema_for_derives(
    file = r#"src/schema.graphql"#,
    module = "schema",
)]
mod queries {
    use super::schema;

    // List Pages
    
    #[derive(cynic::FragmentArguments, Debug)]
    pub struct ListAllPagesArguments {
        pub tags: Option<Vec<String>>,
    }

    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(graphql_type = "Query", argument_struct = "ListAllPagesArguments")]
    pub struct ListAllPages {
        pub pages: Option<PageQuery>,
    }

    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(argument_struct = "ListAllPagesArguments")]
    pub struct PageQuery {
        #[arguments(tags = &args.tags)]
        pub list: Vec<PageListItem>,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    pub struct PageListItem {
        pub id: i32,
        pub path: String,
        pub tags: Option<Vec<Option<String>>>,
        pub title: Option<String>,
    }

    // Tag a Page 
    #[derive(cynic::FragmentArguments, Debug)]
    pub struct TagArguments {
        pub id: i32,
        pub tag: String,
        pub title: String,
    }

    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(graphql_type = "Mutation", argument_struct = "TagArguments")]
    pub struct Tag {
        pub pages: Option<PageTagMutation>,
    }

    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(graphql_type = "PageMutation", argument_struct = "TagArguments")]
    pub struct PageTagMutation {
        #[arguments(title = &args.title, id = &args.id, tag = &args.tag)]
        pub update_tag: Option<DefaultResponse>,
    }

    // Move a Page

    #[derive(cynic::FragmentArguments, Debug)]
    pub struct PageMoveArguments {
        pub destination_path: String,
        pub id: i32,
    }

    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(graphql_type = "Mutation", argument_struct = "PageMoveArguments")]
    pub struct PageMove {
        pub pages: Option<PageMoveMutation>,
    }

    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(graphql_type = "PageMutation", argument_struct = "PageMoveArguments")]
    pub struct PageMoveMutation {
        #[arguments(
            id = &args.id, 
            destination_path = &args.destination_path,
            destination_locale = "en"
        )]
        #[cynic(rename = "move")]
        pub move_: Option<DefaultResponse>,
    }

    #[derive(cynic::QueryFragment, Debug)]
    pub struct DefaultResponse {
        pub response_result: Option<ResponseStatus>,
    }

    #[derive(cynic::QueryFragment, Debug)]
    pub struct ResponseStatus {
        pub succeeded: bool,
        pub slug: String,
        pub message: Option<String>,
        pub error_code: i32,
    }
}

mod schema {
    cynic::use_schema!(r#"src/schema.graphql"#);
}

pub struct ListPages {
    pub pages: Vec<queries::PageListItem>,
    pub pages_returned: usize
}

pub struct TagSuccess {
    pub success_count: usize,
    pub failures: Option<Vec<ResponseStatus>>,
    pub safety_tag: String,
    pub tags: Vec<String>
}

const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION")
);
pub struct Wiki {
    client: reqwest::Client
}

impl Wiki {
    pub fn new(bearer: &'static str) -> Wiki {
        let mut headers = header::HeaderMap::new();
        
        let mut auth_value = header::HeaderValue::from_static(bearer);
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        let client = ClientBuilder::new()
        .http2_prior_knowledge()
        .https_only(true)
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()
        .expect("Failed to initialise http client");
        
        Wiki {
            client
        }
    }

    pub async fn list_pages(&self, prefix: &str, tags: Option<Vec<String>> ) -> Result<ListPages> {
        let op = queries::ListAllPages::build(
            queries::ListAllPagesArguments{tags}
        );
        
        let raw_response = self.client
            .post("https://wiki.redfightback.org/graphql")
            .json(&op)
            .send()
            .await
            .expect("Response had a problem");

        let json = raw_response.json().await.expect("Json decoding issue");

        let response = op.decode_response(json).unwrap();
        
        // unwrap like it's christmas morning
        let page_list = match response.data {
            Some(lap) => match lap.pages {
                Some(pq) => pq.list,
                None => bail!("No pages returned: GraphQlResponse{{data: Some(ListAllPages{{pages: None}}}}")
            }
            None => bail!("No data in response: GraphQlResponse{{data: None}}")
         };

         let pages_returned = page_list.len();

         let filtered_pages = page_list
            .into_iter()
            .filter(|p| {p.path.starts_with(prefix)})
            .sorted_by(|a, b| Ord::cmp(&a.path, &b.path))
            .collect::<Vec<queries::PageListItem>>();

        Ok( ListPages{ pages: filtered_pages, pages_returned})
    }

    // async fn tag_page<'a>(&self, page: &'a queries::PageListItem, tags: Vec<&'a str>) 
    // -> Result<impl Iterator<Item = Operation<'a, queries::Tag>> + 'a> {
        
    //     Ok(ops)
    // }

    pub async fn tag_pages(
        &self, 
        pages: &Vec<queries::PageListItem>, 
        prefix: &str, 
        destination: &str,
        add_tags: Option<Vec<String>>
    ) -> Result<TagSuccess> {
        let page_count = pages.len();
        
        let safety_tag = {
            let mut safety_tag_string = "wikcli-safety-".to_string();
            let mut safety_tag_hash = Sha1::new();
            safety_tag_hash.update(prefix);
            safety_tag_hash.update(destination);
            encode_config_buf(
                safety_tag_hash.finalize(), 
                base64::URL_SAFE_NO_PAD,
                &mut safety_tag_string
            );
            safety_tag_string.truncate(32);
            safety_tag_string
        };

        let mut tags = vec![safety_tag.clone()];

        match add_tags {
            Some(ts) => {tags.extend(ts)},
            None => {}
        }

        // generate an op for each page for each tag
        let ops = pages
            .iter()
            .map(|p| {
                tags.iter().map(|t| {
                    queries::Tag::build(
                        queries::TagArguments{
                            id: p.id, 
                            tag: t.to_string(), 
                            title: t.to_string()
                        }
                    )
                })
            })
            .flatten()
            .collect::<Vec<_>>();

        let requests = ops.iter().map(|op| {
            self.client
                .post("https://wiki.redfightback.org/graphql")
                .json(op)
                .send()
        });

        let raw_responses = join_all(requests).await;

        let (ok, err): (Vec<_>, Vec<_>) = raw_responses.into_iter().partition(|r|r.is_ok());

        match err.len() {
            0 => {} // no errors
            _ => {match ok.len() {
                0 => {bail!("All the requests failed.");}, // all errors
                _ => {bail!("Some, but not all, requests failed. The move may be partially complete.");} 
            }}
        }

        let jsons = join_all(ok.into_iter()
            .map(|r| r.expect("unreachable").json::<cynic::GraphQlResponse<Value>>())).await;

        let (ok, err): (Vec<_>, Vec<_>) = jsons.into_iter().partition(|r|r.is_ok());

        match err.len() {
            0 => {} // no errors
            _ => {match ok.len() {
                0 => {bail!("Deserialising JSON from all responses failed.");}, // all errors
                _ => {bail!("Deserialising JSON from some responses failed. The move may be partially complete.");} 
            }}
        }

        let (ok, err): (Vec<_>, Vec<_>) = ok.into_iter()
            .zip(ops)
            .filter_map(|(j, op)| {
                let tag = op.decode_response(j.unwrap()).unwrap().data;
                
                match tag {
                    Some(t) => match t.pages {
                            Some(ptm) => match ptm.update_tag {
                                Some(dr) => dr.response_result,
                                None => None
                            },
                            None => None
                        }
                    None => None
                    }     
                })
            .partition(|dr| dr.succeeded);

        Ok(TagSuccess{
            success_count: ok.len(), 
            failures: match err.len() {0 => None, _ => Some(err)}, 
            safety_tag,
            tags
         })
    }

    pub async fn move_pages(
        &self, 
        pages: &Vec<queries::PageListItem>, 
        prefix: &str, 
        destination: &str) 
        -> Result<()> {
        
        
        println!("Did not move pages! This bit isn't done yet...");
        
        Ok(())
    }


    /// Check that no pages being moved have `/private/` in the path or `private` tag
    pub async fn safety_check_private<'a>(&self, pages: impl Iterator<Item = &'a queries::PageListItem>)
    -> Option<impl Iterator<Item = &'a queries::PageListItem>> {
        let private_word = "private";
        let private_tag = Some(private_word.to_string());

        let mut private_pages = pages
            .filter(move |p| {
                let is_private_tag = match &p.tags {
                    Some(tags) => tags.contains(&private_tag),
                    None => false
                };
                let is_private_path = p.path.contains(&private_word);
                is_private_tag || is_private_path
            })
            .peekable();

        match private_pages.peek().is_some() {
            true => Some(private_pages),
            false => None
        } 
    }
}