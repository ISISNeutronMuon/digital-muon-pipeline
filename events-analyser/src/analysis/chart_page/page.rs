use crate::{
    analysis::{
        chart_page::chart::{ChartOutput, ChartOutputError},
        metrics::CompletedMetricResult,
    },
    engine::{FlatChart, FlatPage},
};
use build_html::{Html, HtmlContainer, HtmlPage};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum PageOutputError {
    #[error("Json Error: {0}")]
    Json(#[from] serde_json::error::Error),
    #[error("Chart Error: {0}")]
    Chart(#[from] ChartOutputError),
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PageOutput {
    page: FlatPage,
    charts: Vec<ChartOutput>,
}

impl PageOutput {
    pub(crate) fn new(
        page: &FlatPage,
        metrics: &[CompletedMetricResult],
    ) -> Result<Self, PageOutputError> {
        // Get Series Output
        let charts = page
            .charts
            .iter()
            .map(|chart: &FlatChart| ChartOutput::new(chart, metrics))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            page: page.clone(),
            charts,
        })
    }

    pub(crate) fn load_json(path: &Path, chart_name: &str) -> Result<Self, PageOutputError> {
        let mut path = path.to_path_buf();
        path.push(chart_name);
        path.add_extension("json");
        Ok(serde_json::from_reader(File::open(&path)?)?)
    }

    pub(crate) fn save_json(&self, path: &Path) -> Result<(), PageOutputError> {
        let mut path = path.to_owned();
        path.push(&self.page.settings.title);
        path.add_extension("json");
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }

    pub(crate) fn save_plotly(&self, path: &Path) -> Result<(), PageOutputError> {
        const PLOTLY_JS_SCRIPT: &str = "https://cdn.plot.ly/plotly-3.7.0.min.js";

        let html = HtmlPage::new()
            .with_title(&self.page.settings.title)
            .with_script_link(PLOTLY_JS_SCRIPT)
            .with_header(1, &self.page.settings.title);
        let html = self.charts.iter().fold(html, |html, chart| {
            html.with_raw(chart.build_plotly_graph().to_inline_html(None))
        });

        let mut path = path.to_owned();
        path.push(&self.page.settings.title);
        path.add_extension("html");
        let mut file = File::create(path)?;
        file.write_all(html.to_html_string().as_bytes())?;
        file.flush()?;
        Ok(())
    }
}
