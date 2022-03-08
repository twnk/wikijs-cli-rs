use futures::future::join_all;
use cynic::{QueryBuilder, MutationBuilder, serde_json::Value};
use reqwest::{ClientBuilder, header};
use itertools::{Itertools};
use anyhow::{Result, bail};

use queries::{ResponseStatus, PageListItem, ListAllPages, ListAllPagesArguments, MoveSinglePage, MoveSinglePageArguments, GetWikiTitle};

/// Code for Queries generated using <https://generator.cynic-rs.dev/>. 
/// The code generation is currently running an unreleased version with some newer syntax.
/// So manual reverts to the syntax available on the published crate are currenctly necessary.
/// I worked these out manually but later found them documented 
/// <https://github.com/obmarg/cynic/blob/4b5c0fb0d9489bd140be435439f5e29bd5c4ee8b/CHANGELOG.md>
/// 
/// Just paste the schema.graphql content in. It had to be modified to be more
/// compliant. 
/// * Concatenate all the `.graphql` schema files from wiki-js `server/graph/schema`
///   For ease in manual manipulation, place `common.schema` at the top.
/// * You need to remove any `extends Query|Mutation|Subscription { ... }`
///   and add the content of the extend to `Query`, `Mutation` or `Subscription`.
/// * Remove any `@...` annotations and the `directive @auth...` definition.
/// 
/// These are safe schema transformations, as the @ annotations have meaning only 
/// for the server, and inlining the `extends` is a no-op semantically. 
/// 
/// Note it has some finnicky behaviour due to version mismatch. It will generate slightly wrong code. 
/// You may need to make the following substitutions:
/// 
/// * `cynic::FragmentArguments` -> `cynic::QueryVariables`
/// * `#[arguments(name: $name)]` -> `#[arguments(name = &args.name)]`
/// 
/// Specific modifications have been noted inline.
#[cynic::schema_for_derives(
    file = r#"src/schema.graphql"#,
    module = "schema",
)]
mod queries {
    use super::schema;

    // List Pages

    /// (Optional) Tags to filter the list by
    /// 
    /// Codegen Changes
    /// QueryVariables -> FragmentArguments
    #[derive(cynic::FragmentArguments, Debug)]
    pub struct ListAllPagesArguments {
        pub tags: Option<Vec<String>>,
    }

    /// ListAllPages Operation type. Wrapper around PageQuery.
    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(graphql_type = "Query", argument_struct = "ListAllPagesArguments")]
    pub struct ListAllPages {
        pub pages: Option<PageQuery>,
    }

    /// Return (sub)type of Successful Page Query 
    /// 
    /// Codegen Changes
    /// `#[arguments(tags: $tags)]` -> `#[arguments(tags = &args.tags)]`
    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(argument_struct = "ListAllPagesArguments")]
    pub struct PageQuery {
        #[arguments(tags = &args.tags)]
        pub list: Vec<PageListItem>,
    }

    /// Return type for a Single page 
    /// 
    /// The Options ought to be redundant here as a page with no tags
    /// returns an empty Vec rather than a None, and there's no concept of
    /// a None tag that could be returned either, but the Schema doesn't 
    /// express this adequately to Codegen 
    #[derive(cynic::QueryFragment, Debug)]
    pub struct PageListItem {
        pub id: i32,
        pub path: String,
        pub tags: Option<Vec<Option<String>>>,
        pub title: Option<String>,
    }

    // Page Move

    /// Full Destination Path & numeric ID of page
    /// 
    /// Codegen Changes
    /// QueryVariables -> FragmentArguments
    /// Option<String> -> String
    /// Option<i32> -> i32
    #[derive(cynic::FragmentArguments, Debug)]
    pub struct MoveSinglePageArguments {
        pub destination_path: String,
        pub id: i32,
    }

    /// MoveSinglePage Operation type. Wrapper around PageMutation.
    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(graphql_type = "Mutation", argument_struct = "MoveSinglePageArguments")]
    pub struct MoveSinglePage {
        pub pages: Option<PageMutation>,
    }

    /// Return (sub)type of Successful Page Mutation 
    /// 
    /// Codegen Changes
    /// `#[arguments(destinationLocale: "en")]` -> `#[arguments(destination_locale = "en")]`
    /// `#[arguments(destinationPath: $destinationPath)]` -> `#[arguments(destination_path = &args.destination_path)]`
    /// `#[arguments(id: $id)]` -> `#[arguments(id = &args.id)]`
    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(argument_struct = "MoveSinglePageArguments")]
    pub struct PageMutation {
        #[arguments(
            destination_locale = "en", 
            destination_path = &args.destination_path, 
            id = &args.id
        )]
        #[cynic(rename = "move")]
        pub move_: Option<DefaultResponse>,
    }

    /// Return type for MoveSinglePage. Wrapper around ResponseStatus
    #[derive(cynic::QueryFragment, Debug)]
    pub struct DefaultResponse {
        pub response_result: Option<ResponseStatus>,
    }

    /// Return (sub)type for MoveSinglePage.
    #[derive(cynic::QueryFragment, Debug)]
    pub struct ResponseStatus {
        pub error_code: i32,
        pub message: Option<String>,
        pub slug: String,
        pub succeeded: bool,
    }

    // Retrieve Wiki Title
    #[derive(cynic::QueryFragment, Debug)]
    #[cynic(graphql_type = "Query")]
    pub struct GetWikiTitle {
        pub site: Option<SiteQuery>,
    }

    #[derive(cynic::QueryFragment, Debug)]
    pub struct SiteQuery {
        pub config: Option<SiteConfig>,
    }

    #[derive(cynic::QueryFragment, Debug)]
    pub struct SiteConfig {
        pub title: Option<String>,
    }

}

mod schema {
    cynic::use_schema!(r#"src/schema.graphql"#);
}


pub struct ListPages {
    pub pages: Vec<PageListItem>,
    pub pages_returned: usize
}

pub struct MoveSuccess {
    pub success_count: usize,
    pub failures: Option<Vec<ResponseStatus>>
}

const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION")
);
pub struct Wiki {
    client: reqwest::Client,
    endpoint: String
}

pub struct WikiConfig {
    pub api_key: String,
    pub endpoint: String,
    pub http2: bool,
    pub https: bool
}

impl Wiki {
    pub fn new(
        conf: WikiConfig
    ) -> Wiki {
        let mut headers = header::HeaderMap::new();
        
        let bearer = "Bearer ".to_string() + &conf.api_key;

        let mut auth_value = header::HeaderValue::from_str(&bearer).unwrap();
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        let client_builder = ClientBuilder::new()
        .https_only(conf.https)
        .user_agent(USER_AGENT)
        .default_headers(headers);

        let client = match conf.http2 {
            true => {client_builder.http2_prior_knowledge()}
            false => {client_builder}
        }.build().expect("Failed to initialise http client");
        
        Wiki {
            client,
            endpoint: conf.endpoint
        }
    }

    pub async fn get_wiki_title(&self) -> Result<String> {
        let op = GetWikiTitle::build(());
        let raw_response = self.client
            .post(&self.endpoint)
            .json(&op)
            .send()
            .await
            .expect("Response had a problem");

        let json = raw_response.json().await.expect("Json decoding issue");

        let response = op.decode_response(json).unwrap();

        match response.data {
            Some(gwt) => match gwt.site {
                Some(sq) => match sq.config {
                    Some(sc) => match sc.title {
                        Some(t) => Ok(t),
                        None => bail!("No title"),
                    },
                    None => bail!("No config returned"),
                },
                None => bail!("No site returned")
            }
            None => bail!("No data in response")
         }
    }

    pub async fn list_pages(&self, prefix: &str, tags: Option<Vec<String>> ) -> Result<ListPages> {
        let op = ListAllPages::build(
            ListAllPagesArguments{tags}
        );
        
        let raw_response = self.client
            .post(&self.endpoint)
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

    pub async fn move_pages(
        &self, 
        pages: &Vec<queries::PageListItem>, 
        prefix: &str, 
        destination: &str,
    ) -> Result<MoveSuccess> {

        let trim = prefix.len();

        // generate an op for each page
        let ops = pages
            .iter()
            .map(|p| {
                MoveSinglePage::build(
                    MoveSinglePageArguments{
                        id: p.id, 
                        destination_path: destination.to_owned() + &p.path[trim..]
                    }
                )
            })
            .collect::<Vec<_>>();

        let requests = ops.iter().map(|op| {
            self.client
                .post(&self.endpoint)
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
                            Some(ptm) => match ptm.move_ {
                                Some(dr) => dr.response_result,
                                None => None
                            },
                            None => None
                        }
                    None => None
                    }     
                })
            .partition(|r| r.succeeded);

        Ok(MoveSuccess{
            success_count: ok.len(), 
            failures: match err.len() {0 => None, _ => Some(err)}
         })
    }


    /// Check that no pages being moved have `/private/` in the path or `private` tag
    pub async fn safety_check_private<'a>(&self, pages: impl Iterator<Item = &'a PageListItem>)
    -> Option<impl Iterator<Item = &'a PageListItem>> {
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