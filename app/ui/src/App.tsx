// The workspace: title bar, classic menu bar, toolbar, left rail, document tabs (Data, Output) and the context bar.
// Layout and look follow the approved mockup and the shared Prism style (D02, D13).

import { useCallback, useEffect, useRef, useState } from 'react';
import { call, inTauri } from './bridge';
import { DataView, type FindRequest } from './components/DataView';
import { SettingsDialog } from './components/SettingsDialog';
import { ExamplesDialog } from './components/ExamplesDialog';
import { getSettings, onSettings } from './settings';
import { AnalysisDialog } from './components/AnalysisDialog';
import { SemBuilder } from './components/SemBuilder';
import { CfaDialog } from './components/CfaDialog';
import { DesignDialog } from './components/DesignDialog';
import { DataHealthDialog } from './components/DataHealthDialog';
import { JobProgress } from './components/JobProgress';
import { ChooserPanel } from './components/ChooserPanel';
import { HelpContext, HelpDialog } from './components/HelpDialog';
import { helpFor } from './help';
import { RecoveryDialog, type RecoveryCopy } from './components/RecoveryDialog';
import { AddCasesDialog, AddVariablesDialog, AggregateDialog, RestructureDialog } from './components/DataToolDialogs';
import { AiSettingsDialog } from './components/AiWriter';
import { AssistantDialog } from './components/AssistantDialog';
import { PowerDialog } from './components/PowerDialog';
import { ScriptView } from './components/ScriptView';
import { ReportView } from './components/ReportView';
import { DescribeDialog, type DescribeKind } from './components/DescribeDialog';
import { Icon } from './components/Icon';
import { MenuBar, type Menu } from './components/MenuBar';
import { OutputView } from './components/OutputView';
import { TTestDialog } from './components/TTestDialog';
import { ComputeDialog, RecodeDialog, ScaleDialog } from './components/TransformDialogs';
import { DateDialog, RankDialog, ReverseDialog, SortDialog, StandardizeDialog } from './components/MoreTransforms';
import { DataSettingsDialog, settingChips, type DataSettings } from './components/DataSettingsDialog';
import { VariableView } from './components/VariableView';
import logo from './assets/PrismStatistics.svg';
import { GoogleSheetDialog, PathDialog, SheetDialog, UnsavedDialog } from './components/Modal';
import { chooseExportPath, chooseProjectToOpen, chooseProjectToSave, download, fileToBase64, PROJECT_EXT } from './files';
import { ANALYSES, dialogFor, restore } from './analyses';
import type { AiStatus, DatasetInfo, ImportReport, OutputItem, ProjectState, TTestSpec } from './types';

type OpenDataReply = { choose_sheet: string[] } | { report: ImportReport; dataset: DatasetInfo; state: ProjectState };

type Screen = 'home' | 'data' | 'output' | 'report' | 'script';
type DataTab = 'data' | 'variables';

export function App() {
  const [dataset, setDataset] = useState<DatasetInfo | null>(null);
  const [cases, setCases] = useState<DataSettings>({});
  // Select, split and weight settings belong to the open project; read them whenever the data change.
  useEffect(() => {
    if (!dataset) return setCases({});
    call<DataSettings>('data_settings').then(setCases, () => setCases({}));
  }, [dataset]);
  const [project, setProject] = useState<ProjectState | null>(null);
  const [screen, setScreen] = useState<Screen>('home');
  const [dataTab, setDataTab] = useState<DataTab>('data');
  const [output, setOutput] = useState<OutputItem[]>([]);
  const [dialog, setDialog] = useState<string | null>(null);
  const [recovery, setRecovery] = useState<RecoveryCopy[]>([]);
  // The help viewer: undefined when closed, else the page to start on (null for the first chapter).
  const [help, setHelp] = useState<string | null | undefined>(undefined);
  // Help me choose, docked on the right.
  const [choosing, setChoosing] = useState(false);
  // Edit › Find and Go to, passed to Data View.
  const [findReq, setFindReq] = useState<FindRequest | null>(null);
  // Tools › Settings: the text size applies to the whole window at once.
  const [textSize, setTextSize] = useState(() => getSettings().textSize);
  useEffect(() => onSettings((s) => setTextSize(s.textSize)), []);
  useEffect(() => {
    (document.body.style as CSSStyleDeclaration & { zoom: string }).zoom = textSize === 100 ? '' : String(textSize / 100);
  }, [textSize]);
  // The arguments of a result being reopened from its heading (Settings…); cleared whenever the dialog closes.
  const [initialArgs, setInitialArgs] = useState<Record<string, unknown> | null>(null);
  useEffect(() => {
    if (!dialog) setInitialArgs(null);
  }, [dialog]);
  // The script the Paste buttons write to (Script screen).
  const [script, setScript] = useState('');
  const paste = (command: string, args: unknown) => {
    setScript((s) => `${s.replace(/\n*$/, s ? '\n' : '')}${command} ${JSON.stringify(args)}\n`);
    setDialog(null);
    say('Pasted into the script (Script, on the left). Nothing was run.');
  };
  const reopen = (it: OutputItem) => {
    const id = dialogFor(it.command, it.args ?? {});
    if (!(id === 'ttest' || id === 'frequencies' || id === 'descriptives' || id === 'power' || ANALYSES[id])) return;
    setInitialArgs(it.args ?? {});
    setDialog(id);
  };
  const [sheetChoice, setSheetChoice] = useState<{ fileName: string; sheets: string[]; pick: (sheet: string) => void } | null>(null);
  const [googleOpen, setGoogleOpen] = useState(false);
  const [ai, setAi] = useState<AiStatus | null>(null);
  useEffect(() => {
    call<AiStatus>('ai_status').then(setAi).catch(() => undefined);
  }, []);
  const [pathPrompt, setPathPrompt] = useState<{ mode: 'open' | 'save'; resolve: (p: string | null) => void } | null>(null);
  const [unsaved, setUnsaved] = useState<{ resolve: (go: boolean) => void } | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const say = useCallback((msg: string) => {
    setToast(msg);
    window.setTimeout(() => setToast((t) => (t === msg ? null : t)), 3600);
  }, []);

  /** Brings the title bar and the output's out-of-date marks up to date after a change. */
  const refresh = useCallback(async () => {
    setProject(await call<ProjectState>('project_state'));
    setOutput(await call<OutputItem[]>('results'));
  }, []);

  /** After a Transform change: the new variables, the title bar, and the out-of-date marks. */
  const dataChanged = async (message: string) => {
    setDataset(await call<DatasetInfo>('dataset'));
    await refresh();
    say(message);
  };

  /** In a browser there is no file dialog, so ask for a path. */
  const askPath = (mode: 'open' | 'save') => new Promise<string | null>((resolve) => setPathPrompt({ mode, resolve }));

  const saveProject = async (as: boolean): Promise<boolean> => {
    try {
      let path: string | null | undefined = as ? undefined : project?.path ?? undefined;
      if (!path) {
        const suggested = project?.name ?? dataset?.name ?? 'Untitled';
        path = await chooseProjectToSave(suggested);
        if (path === undefined) path = await askPath('save');
        if (!path) return false;
      }
      const state = await call<ProjectState>('save_project', { path });
      setProject(state);
      say(`Saved ${state.path}`);
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    }
  };

  /** Before replacing the open project: offer to save unsaved changes. Resolves false if the user cancels. */
  const okToReplace = async (): Promise<boolean> => {
    if (!project?.dirty) return true;
    const choice = await new Promise<boolean | 'save'>((resolve) => setUnsaved({ resolve: (go) => resolve(go) }));
    if (choice === 'save') return saveProject(false);
    return choice;
  };

  /** File › Open Example Data: a built-in teaching dataset, as new unsaved data. */
  const openExample = async (id: string) => {
    if (!(await okToReplace())) return;
    try {
      setError(null);
      setDialog(null);
      setHelp(undefined);
      const r = await call<Exclude<OpenDataReply, { choose_sheet: string[] }>>('open_example', { id });
      opened(r, r.dataset.name);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const openFile = async () => {
    if (await okToReplace()) fileRef.current?.click();
  };

  /** Opens a project: the one given (a double-clicked .pstat file), or one the user chooses. */
  const openProject = async (given?: string) => {
    let path: string | null | undefined = given;
    if (!path) {
      if (!(await okToReplace())) return;
      path = await chooseProjectToOpen();
      if (path === undefined) path = await askPath('open');
      if (!path) return;
    }
    try {
      setError(null);
      const r = await call<{ state: ProjectState; dataset: DatasetInfo; results: OutputItem[] }>('open_project', { path });
      setProject(r.state);
      setDataset(r.dataset);
      setOutput(r.results);
      setScreen(r.results.length ? 'output' : 'data');
      const stale = r.results.filter((x) => x.stale).length;
      say(`Opened ${r.state.name}${stale ? `. ${stale} result(s) are out of date.` : '.'}`);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const loadData = async (file: File, sheet?: string) => {
    setError(null);
    try {
      const base64 = await fileToBase64(file);
      const r = await call<OpenDataReply>('open_bytes', { file_name: file.name, base64, sheet });
      if ('choose_sheet' in r) {
        setSheetChoice({ fileName: file.name, sheets: r.choose_sheet, pick: (s) => loadData(file, s) });
        return;
      }
      opened(r, file.name);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  /** Opens an online spreadsheet shared as "anyone with the link". Throws so the dialog can show the reason. */
  const loadGoogle = async (url: string, sheet?: string) => {
    setError(null);
    const r = await call<OpenDataReply>('open_google_sheet', { url, sheet });
    setGoogleOpen(false);
    if ('choose_sheet' in r) {
      setSheetChoice({ fileName: 'the online spreadsheet', sheets: r.choose_sheet, pick: (s) => loadGoogle(url, s).catch((e: Error) => setError(e.message)) });
      return;
    }
    opened(r, r.dataset.name);
  };

  /** Shows newly opened data and what happened during the import. */
  const opened = (r: Exclude<OpenDataReply, { choose_sheet: string[] }>, name: string) => {
    setDataset(r.dataset);
    setProject(r.state);
    setOutput([]);
    setScreen('data');
    setDataTab('data');
    const rep = r.report;
    const extra = [
      rep.renamed.length ? `${rep.renamed.length} column name(s) were adjusted to be valid variable names.` : '',
      rep.sheet ? `Sheet "${rep.sheet}".` : '',
      ...rep.notes,
    ].filter(Boolean);
    say(`Opened ${name}: ${rep.rows} cases, ${rep.variables} variables. ${extra.join(' ')}`.trim());
  };

  // File › Import › Value labels from a format catalog (.sas7bcat), for the open .sas7bdat data.
  const catalogRef = useRef<HTMLInputElement>(null);
  const onCatalog = async (file: File | undefined) => {
    if (!file) return;
    try {
      const r = await call<{ notes: string[]; dataset: DatasetInfo }>('load_sas_catalog', { file_name: file.name, base64: await fileToBase64(file) });
      setDataset(r.dataset);
      say(r.notes.join(' '));
      refresh();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const onFile = async (file: File | undefined) => {
    if (fileRef.current) fileRef.current.value = '';
    if (file) await loadData(file);
  };

  const keys = useRef<(e: KeyboardEvent) => void>(() => {});
  keys.current = (e: KeyboardEvent) => {
    if (e.key === 'F1') {
      e.preventDefault();
      setHelp(helpFor(dialog)?.id ?? null);
      return;
    }
    if (!e.ctrlKey) return;
    const k = e.key.toLowerCase();
    // In a text box, Ctrl+Z and Ctrl+Y undo typing, as everywhere else.
    const el = e.target as HTMLElement | null;
    const typing = !!el && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName));
    if ((k === 'z' || k === 'y') && !typing && !dialog) {
      e.preventDefault();
      undoRedo(k === 'z' && !e.shiftKey);
      return;
    }
    if ((k === 'f' || k === 'g') && !e.shiftKey && !dialog && dataset) {
      e.preventDefault();
      find(k === 'f' ? 'find' : 'goto');
      return;
    }
    if (k === 'h' && e.shiftKey) {
      e.preventDefault();
      setChoosing((c) => !c);
      return;
    }
    if (k === 'o' && !e.shiftKey) {
      e.preventDefault();
      openFile();
    } else if (k === 'o' && e.shiftKey) {
      e.preventDefault();
      openProject();
    } else if (k === 's' && dataset) {
      e.preventDefault();
      saveProject(e.shiftKey);
    }
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => keys.current(e);
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  useEffect(() => {
    // Warn before the window or tab closes with unsaved changes.
    const onUnload = (e: BeforeUnloadEvent) => {
      if (project?.dirty) e.preventDefault();
    };
    window.addEventListener('beforeunload', onUnload);
    return () => window.removeEventListener('beforeunload', onUnload);
  }, [project?.dirty]);

  /** Exports every result in the output: through a save dialog in the desktop app, as a download in a browser. */
  const exportOutput = async (format: 'docx' | 'html' | 'pdf' | 'r', report = false) => {
    try {
      const base = `${project?.name ?? dataset?.name ?? 'PrismStatistics'} ${format === 'r' ? 'analysis' : 'output'}`;
      const path = await chooseExportPath(format, base);
      if (path === null) return;
      if (path === undefined) {
        const r = await call<{ file_name: string; base64: string; count: number }>('export', { format, report });
        download(r.file_name, r.base64, format === 'docx' ? 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' : format === 'pdf' ? 'application/pdf' : format === 'r' ? 'text/plain' : 'text/html');
        say(format === 'r' ? `Wrote the R script ${r.file_name}.` : `Exported ${r.count} result${r.count === 1 ? '' : 's'} to ${r.file_name}.`);
      } else {
        const r = await call<{ path: string; count: number }>('export', { format, path, report });
        say(format === 'r' ? `Wrote the R script to ${r.path}.` : `Exported ${r.count} result${r.count === 1 ? '' : 's'} to ${r.path}.`);
      }
    } catch (e) {
      setError((e as Error).message);
    }
  };

  // Work left unsaved by a window that closed or crashed is offered once, at start-up.
  useEffect(() => {
    call<RecoveryCopy[]>('recovery_list')
      .then(setRecovery)
      .catch(() => undefined);
  }, []);

  // A project double-clicked in File Explorer arrives as the program's argument.
  useEffect(() => {
    if (!inTauri()) return;
    import('@tauri-apps/api/core')
      .then(({ invoke }) => invoke<string | null>('opened_with'))
      .then((path) => {
        if (path) openProject(path);
      })
      .catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Edit › Undo (back) and Redo: the data, the record and the output go back one step. */
  const undoRedo = async (back: boolean) => {
    try {
      const r = await call<{ did: string; dataset: DatasetInfo }>(back ? 'undo' : 'redo');
      setDataset(r.dataset);
      await refresh();
      say(`${back ? 'Undone' : 'Redone'}: ${r.did}.`);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const find = (mode: 'find' | 'goto') => {
    if (!dataset) return;
    setScreen('data');
    setDataTab('data');
    setFindReq((f) => ({ mode, n: (f?.n ?? 0) + 1 }));
  };

  const needData = !dataset;
  const later = (label: string) => ({ label, later: true });
  const open = (id: string, label: string) => ({ label, onSelect: () => setDialog(id), disabled: needData });
  const menus: Menu[] = [
    {
      label: 'File',
      items: [
        { label: 'Open data…', shortcut: 'Ctrl+O', onSelect: openFile },
        { label: 'Open project…', shortcut: 'Ctrl+Shift+O', onSelect: () => openProject() },
        { label: 'Open Example Data…', onSelect: () => setDialog('examples') },
        '-',
        { label: 'Save project', shortcut: 'Ctrl+S', onSelect: () => saveProject(false), disabled: needData },
        { label: 'Save project as…', shortcut: 'Ctrl+Shift+S', onSelect: () => saveProject(true), disabled: needData },
        '-',
        { label: 'Import', submenu: [{ label: 'CSV or TSV file…', onSelect: openFile }, { label: 'Spreadsheet (.xlsx)…', onSelect: openFile }, { label: '.sav, .dta or .sas7bdat file…', onSelect: openFile }, { label: '.jasp file (data only)…', onSelect: openFile }, { label: 'Online spreadsheet (shared link)…', onSelect: async () => (await okToReplace()) && setGoogleOpen(true) }, { label: 'Value labels from a format catalog (.sas7bcat)…', onSelect: () => catalogRef.current?.click(), disabled: needData }] },
        { label: 'Export', submenu: [{ label: 'Word (.docx)…', onSelect: () => exportOutput('docx'), disabled: !output.length }, { label: 'HTML…', onSelect: () => exportOutput('html'), disabled: !output.length }, { label: 'PDF (tagged, accessible)…', onSelect: () => exportOutput('pdf'), disabled: !output.length }, { label: 'R script…', onSelect: () => exportOutput('r'), disabled: needData }] },
      ],
    },
    {
      label: 'Edit',
      items: [
        { label: project?.undo ? `Undo ${project.undo}` : 'Undo', shortcut: 'Ctrl+Z', onSelect: () => undoRedo(true), disabled: !project?.undo },
        { label: project?.redo ? `Redo ${project.redo}` : 'Redo', shortcut: 'Ctrl+Y', onSelect: () => undoRedo(false), disabled: !project?.redo },
        '-',
        { label: 'Find…', shortcut: 'Ctrl+F', onSelect: () => find('find'), disabled: needData },
        { label: 'Go to Case or Variable…', shortcut: 'Ctrl+G', onSelect: () => find('goto'), disabled: needData },
      ],
    },
    { label: 'View', items: [{ label: 'Data View', onSelect: () => (setScreen('data'), setDataTab('data')), disabled: needData }, { label: 'Variable View', onSelect: () => (setScreen('data'), setDataTab('variables')), disabled: needData }, { label: 'Output', onSelect: () => setScreen('output') }] },
    { label: 'Data', items: [open('sort', 'Sort Cases…'), '-', open('cases_filter', 'Select Cases…'), open('cases_split', 'Split File…'), open('cases_weight', 'Weight Cases…'), { label: 'Merge Files', submenu: [open('add_cases', 'Add Cases…'), open('add_variables', 'Add Variables…')] }, open('aggregate', 'Aggregate…'), open('restructure', 'Restructure…'), open('health', 'Data Health…')] },
    { label: 'Transform', items: [open('compute', 'Compute Variable…'), open('recode', 'Recode…'), open('reverse', 'Reverse Score…'), open('scale', 'Scale Score…'), '-', open('rank', 'Rank Cases…'), open('standardize', 'Standardize (z-scores)…'), open('dates', 'Date and Time…')] },
    {
      label: 'Analyze',
      items: [
        { label: 'Help me choose…', shortcut: 'Ctrl+Shift+H', onSelect: () => setChoosing(true) },
        '-',
        { label: 'Descriptive Statistics', submenu: [{ label: 'Frequencies…', onSelect: () => setDialog('frequencies'), disabled: needData }, { label: 'Descriptives…', onSelect: () => setDialog('descriptives'), disabled: needData }, open('explore', 'Explore…'), open('bootstrap', 'Bootstrap…'), open('crosstabs', 'Crosstabs…')] },
        {
          label: 'Compare Means',
          submenu: [open('ttest_one_sample', 'One-Sample T Test…'), { label: 'Independent-Samples T Test…', onSelect: () => setDialog('ttest'), disabled: needData }, open('ttest_paired', 'Paired-Samples T Test…'), open('anova_oneway', 'One-Way ANOVA…'), open('yuen', 'Yuen’s Test (Trimmed Means)…'), open('yuen_paired', 'Paired Yuen Test…'), open('trimmed_anova', 'Robust ANOVA (Trimmed Means)…'), open('anova_factorial', 'Factorial ANOVA (Type III)…'), open('rm_anova', 'Repeated-Measures ANOVA…'), open('manova', 'MANOVA…')],
        },
        { label: 'Correlate', submenu: [open('correlation', 'Bivariate…'), open('partial_correlation', 'Partial…'), open('canonical', 'Canonical…')] },
        { label: 'Regression', submenu: [open('regression', 'Linear…'), open('curve', 'Curve Estimation…'), open('mediation', 'Mediation…'), open('moderation', 'Moderation…'), open('logistic', 'Binary Logistic…'), open('ordinal_regression', 'Ordinal Logistic…'), open('multinomial_regression', 'Multinomial Logistic…'), open('count_regression', 'Poisson and Negative Binomial…'), open('panel', 'Panel Data…'), open('pgmm', 'Dynamic Panel (GMM)…'), open('penalized', 'Penalized (Lasso, Ridge, Elastic Net)…')] },
        { label: 'Structural Equation Modeling', submenu: [open('cfa', 'Factor Analysis of a Scale (CFA)…'), open('sem', 'Structural Equation Model…'), open('sem_invariance', 'Measurement Invariance…'), open('pls', 'PLS-SEM…')] },
        { label: 'Complex Samples', submenu: [open('survey_estimates', 'Survey Estimates…'), open('survey_regression', 'Survey Regression…')] },
        { label: 'Time Series', submenu: [open('ts_acf', 'Autocorrelation…'), open('ts_smooth', 'Smoothing…'), open('arima', 'ARIMA…')] },
        { label: 'Survival', submenu: [open('kaplan_meier', 'Kaplan-Meier…'), open('cox_regression', 'Cox Regression…')] },
        { label: 'Classify', submenu: [open('roc', 'ROC Curve…'), open('lca', 'Latent Class Analysis…'), open('lpa', 'Latent Profile Analysis…'), open('tree', 'Decision Tree…'), open('forest', 'Random Forest…'), open('boost', 'Gradient Boosting…')] },
        { label: 'Quality Control', submenu: [open('spc', 'Control Charts…')] },
        { label: 'Design of Experiments', submenu: [{ label: 'Create Design…', onSelect: () => setDialog('design') }, open('doe_effects', 'Factorial Effects…'), open('rsm', 'Response Surface…'), open('doe_mixture', 'Mixture Model…')] },
        open('meta', 'Meta-Analysis…'),
        { label: 'Permutation Tests', submenu: [open('perm_two_groups', 'Two Groups…'), open('perm_paired', 'Paired Measures…'), open('perm_correlation', 'Correlation…'), open('perm_anova', 'One-Way ANOVA…')] },
        { label: 'Missing Values', submenu: [open('missing_patterns', 'Missing Data Patterns…'), open('mi_regression', 'Regression with Multiple Imputation…')] },
        { label: 'Mixed Models', submenu: [open('mixed_model', 'Linear Mixed Model…'), open('gee', 'Generalized Estimating Equations…')] },
        { label: 'Dimension Reduction', submenu: [open('factor', 'Factor Analysis…')] },
        { label: 'Scale', submenu: [open('reliability', 'Reliability Analysis…'), open('kappa', 'Cohen’s Kappa…'), open('fleiss', 'Fleiss’ Kappa…'), open('kripp', 'Krippendorff’s Alpha…'), open('icc', 'Intraclass Correlation…'), open('irt', 'Item Response Theory…')] },
        {
          label: 'Nonparametric Tests',
          submenu: [open('mann_whitney', 'Mann-Whitney U (2 independent samples)…'), open('wilcoxon', 'Wilcoxon Signed-Rank (2 related samples)…'), open('kruskal', 'Kruskal-Wallis (k independent samples)…'), open('friedman', 'Friedman (k related samples)…'), open('mcnemar', 'McNemar (2 related yes/no measures)…'), open('cochran', 'Cochran’s Q (k related yes/no measures)…'), open('sign_test', 'Sign Test (2 related samples)…'), open('median_test', 'Median Test (k independent samples)…'), open('ks2', 'Kolmogorov-Smirnov (2 independent samples)…'), open('chisq_gof', 'Chi-Square (goodness of fit)…'), open('binomial', 'Binomial…'), open('runs', 'Runs…')],
        },
        '-',
        { label: 'Power Analysis…', onSelect: () => setDialog('power') },
      ],
    },
    { label: 'Graphs', items: [open('chart_histogram', 'Histogram…'), open('chart_boxplot', 'Box Plot…'), open('chart_bar', 'Bar Chart…'), open('chart_scatter', 'Scatterplot…'), open('chart_qq', 'Normal Q-Q Plot…')] },
    { label: 'Results', items: [{ label: 'Clear output', disabled: needData, onSelect: async () => (setOutput(await call<OutputItem[]>('clear_results')), refresh()) }] },
    { label: 'Tools', items: [{ label: 'AI writing…', onSelect: () => setDialog('ai') }, { label: 'Connect an AI assistant…', onSelect: () => setDialog('mcp') }, '-', { label: 'Settings…', onSelect: () => setDialog('settings') }] },
    {
      label: 'Help',
      items: [
        { label: 'PrismStatistics Help', shortcut: 'F1', onSelect: () => setHelp(null) },
        { label: 'Help me choose…', onSelect: () => setChoosing(true) },
        { label: 'Getting Started', onSelect: () => setHelp('getting-started') },
        { label: 'Worked Examples', onSelect: () => setHelp('two-groups') },
        { label: 'Glossary', onSelect: () => setHelp('glossary') },
        { label: 'Saving, Backups and Recovery', onSelect: () => setHelp('saving') },
        { label: 'About PrismStatistics', onSelect: () => setDialog('about') },
      ],
    },
  ];

  const rail: { id: Screen; label: string; icon: string }[] = [
    { id: 'home', label: 'Home', icon: 'home' },
    { id: 'data', label: 'Data', icon: 'data' },
    { id: 'output', label: 'Output', icon: 'output' },
    { id: 'report', label: 'Report', icon: 'report' },
    { id: 'script', label: 'Script', icon: 'syntax' },
  ];

  return (
    <HelpContext.Provider value={{ dialog, show: (id) => setHelp(id) }}>
    <div className="app">
      <header className="titlebar">
        <span className="brand">
          <img src={logo} alt="" />
          <span>PrismStatistics</span>
        </span>
        <span className="sep" aria-hidden="true" />
        <span className="proj" title={project?.path ?? undefined}>
          {project?.name ?? (dataset ? dataset.name : 'No data open')}
        </span>
        <span className="sep" aria-hidden="true" />
        <span className="proj">{rail.find((r) => r.id === screen)?.label}</span>
        {project && <span className={`saved ${project.dirty ? 'unsaved' : ''}`}>{project.dirty ? (project.path ? 'Unsaved changes' : 'Not saved yet') : 'Saved'}</span>}
      </header>
      <MenuBar menus={menus} />
      <div className="toolbar" role="toolbar" aria-label="Quick actions">
        <button type="button" className="tb primary" onClick={openFile}>
          <Icon name="opendata" />
          <span>Open data</span>
        </button>
        <button type="button" className="tb" disabled={needData} onClick={() => saveProject(false)} title="Save project (Ctrl+S)">
          <Icon name="save" />
          <span>Save</span>
        </button>
        <span className="tbsep" />
        <button type="button" className="tb" disabled={needData} onClick={() => setDialog('frequencies')}>
          <Icon name="chart" />
          <span>Frequencies</span>
        </button>
        <button type="button" className="tb" disabled={needData} onClick={() => setDialog('descriptives')}>
          <Icon name="vars" />
          <span>Descriptives</span>
        </button>
        <button type="button" className="tb" disabled={needData} onClick={() => setDialog('ttest')}>
          <Icon name="compare" />
          <span>T test</span>
        </button>
        <span className="tbsep" />
        <button type="button" className="tb" aria-pressed={choosing} onClick={() => setChoosing((c) => !c)} title="Help me choose (Ctrl+Shift+H)">
          <Icon name="bulb" />
          <span>Help me choose</span>
        </button>
      </div>
      <div className="body">
        <nav className="rail" aria-label="Workspace">
          {rail.map((r) => (
            <button type="button" key={r.id} aria-current={screen === r.id ? 'page' : undefined} onClick={() => setScreen(r.id)}>
              <Icon name={r.icon} />
              <span className="lbl">{r.label}</span>
            </button>
          ))}
        </nav>
        <main className="main">
          {screen === 'data' && dataset && (
            <div className="doctabs" role="tablist" aria-label="Data">
              <button type="button" role="tab" aria-selected={dataTab === 'data'} onClick={() => setDataTab('data')}>
                Data View
              </button>
              <button type="button" role="tab" aria-selected={dataTab === 'variables'} onClick={() => setDataTab('variables')}>
                Variable View
              </button>
            </div>
          )}
          <div className="view" tabIndex={-1}>
            {error && (
              <div className="errbox" role="alert" style={{ marginBottom: 12 }}>
                {error}
              </div>
            )}
            {screen === 'home' && (
              <div className="welcome">
                <div className="hero">
                  <img src={logo} alt="" />
                  <div>
                    <h1>PrismStatistics</h1>
                    <p>Familiar statistics, explained. Part of the PrismOpenResearch suite.</p>
                  </div>
                </div>
                <div className="tiles">
                  <button type="button" className="tile primary" onClick={openFile}>
                    <Icon name="opendata" />
                    Open data
                    <small>CSV, .xlsx, .sav, .dta, .sas7bdat, .jasp</small>
                  </button>
                  <button type="button" className="tile" onClick={() => openProject()}>
                    <Icon name="folder" />
                    Open project
                    <small>.{PROJECT_EXT} file</small>
                  </button>
                  <button type="button" className="tile" disabled={needData} onClick={() => setDialog('ttest')}>
                    <Icon name="compare" />
                    Compare two groups
                    <small>Independent-samples t test</small>
                  </button>
                  <button type="button" className="tile" onClick={() => setDialog('examples')}>
                    <Icon name="data" />
                    Example data
                    <small>Practice datasets and worked examples</small>
                  </button>
                  <button type="button" className="tile" onClick={() => setChoosing(true)}>
                    <Icon name="bulb" />
                    Help me choose
                    <small>Not sure which analysis? Answer a few questions</small>
                  </button>
                  <button type="button" className="tile" onClick={() => setScreen('output')}>
                    <Icon name="output" />
                    Output
                    <small>{output.length} results</small>
                  </button>
                </div>
              </div>
            )}
            {screen === 'data' &&
              (dataset ? (
                dataTab === 'data' ? <DataView dataset={dataset} onEdited={refresh} onGrew={dataChanged} request={findReq} /> : <VariableView dataset={dataset} onChange={(d) => (setDataset(d), refresh())} />
              ) : (
                <div className="empty-state">
                  <Icon name="data" />
                  <h2>No data open</h2>
                  <button type="button" className="btn primary" onClick={openFile}>
                    Open data…
                  </button>
                </div>
              ))}
            {screen === 'report' && <ReportView results={output} onExport={(f) => exportOutput(f, true)} />}
            {screen === 'script' && <ScriptView text={script} setText={setScript} onRan={(r) => (setOutput(r), refresh())} say={say} />}
            {screen === 'output' && <OutputView items={output} onCopied={say} onChanged={refresh} onExport={exportOutput} onEdit={reopen} aiReady={!!ai?.enabled && !!ai?.installed} />}
          </div>
        </main>
        {choosing && (
          <ChooserPanel
            dataset={dataset}
            onClose={() => setChoosing(false)}
            onHelp={(id) => setHelp(id)}
            onOpen={(id, args) => {
              setInitialArgs(args);
              setDialog(id);
              if (Object.keys(args).length && id !== 'power') say('Variables filled in from Help me choose. Check them, then run.');
            }}
          />
        )}
      </div>
      <footer className="ctxbar" aria-label="Analysis context">
        {dataset ? (
          <>
            <span>
              Dataset <b>{dataset.name}</b>
            </span>
            <span className="chip">{dataset.rows.toLocaleString()} cases</span>
            {settingChips(cases).map((text, i) => (
              <button key={i} type="button" className={`chip${text.startsWith('No ') ? '' : ' on'}`} onClick={() => setDialog(["cases_filter", "cases_weight", "cases_split"][i])} aria-label={`${text}: change which cases the analyses use`}>
                {text}
              </button>
            ))}
          </>
        ) : (
          <span>Open a data file to begin</span>
        )}
      </footer>
      <input ref={catalogRef} type="file" accept=".sas7bcat" hidden onChange={(e) => (onCatalog(e.target.files?.[0]), (e.target.value = ''))} aria-label="Choose a format catalog (.sas7bcat)" />
      <input ref={fileRef} type="file" accept=".csv,.tsv,.txt,.xlsx,.xlsm,.sav,.zsav,.dta,.sas7bdat,.jasp,text/csv" hidden onChange={(e) => onFile(e.target.files?.[0])} aria-label="Choose a data file" />
      {dialog?.startsWith('cases_') && dataset && (
        <DataSettingsDialog
          dataset={dataset}
          initial={cases}
          focus={dialog.slice(6) as 'filter' | 'split' | 'weight'}
          onPaste={paste}
          onClose={() => setDialog(null)}
          onDone={(s) => (setCases(s), refresh(), say('Analyses run from now on use these cases. Earlier results keep theirs.'))}
        />
      )}
      <JobProgress />
      {recovery.length > 0 && (
        <RecoveryDialog
          copies={recovery}
          onClose={() => setRecovery([])}
          onRecovered={(r) => {
            setRecovery([]);
            setProject(r.state);
            setDataset(r.dataset);
            setOutput(r.results);
            setScreen(r.results.length ? 'output' : 'data');
            say(`Recovered ${r.state.name}. Save it to keep it.`);
          }}
        />
      )}
      {dialog === 'health' && dataset && <DataHealthDialog onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'add_cases' && dataset && <AddCasesDialog dataset={dataset} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'add_variables' && dataset && <AddVariablesDialog dataset={dataset} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'aggregate' && dataset && <AggregateDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'restructure' && dataset && <RestructureDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'compute' && dataset && <ComputeDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'recode' && dataset && <RecodeDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'scale' && dataset && <ScaleDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'sort' && dataset && <SortDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'rank' && dataset && <RankDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'standardize' && dataset && <StandardizeDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'reverse' && dataset && <ReverseDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'dates' && dataset && <DateDialog dataset={dataset} onPaste={paste} onClose={() => setDialog(null)} onDone={dataChanged} />}
      {dialog === 'ttest' && dataset && (
        <TTestDialog
          dataset={dataset}
          initial={(initialArgs as Partial<TTestSpec>) ?? undefined}
          onPaste={paste}
          onClose={() => setDialog(null)}
          onDone={(items) => {
            setOutput((o) => [...o, ...items]);
            setDialog(null);
            setScreen('output');
            refresh();
          }}
        />
      )}
      {(dialog === 'frequencies' || dialog === 'descriptives') && dataset && (
        <DescribeDialog
          kind={dialog}
          dataset={dataset}
          initial={(initialArgs as { variables?: string[]; statistics?: string[] }) ?? undefined}
          onPaste={paste}
          onClose={() => setDialog(null)}
          onDone={(items) => {
            setOutput((o) => [...o, ...items]);
            setDialog(null);
            setScreen('output');
            refresh();
          }}
        />
      )}
      {dialog === 'cfa' && dataset && (
        <CfaDialog
          dataset={dataset}
          onPaste={paste}
          onClose={() => setDialog(null)}
          onOpenBuilder={(args) => {
            setInitialArgs(args);
            setDialog('sem');
          }}
          onDone={(items) => {
            setOutput((o) => [...o, ...items]);
            setDialog(null);
            setScreen('output');
            refresh();
          }}
        />
      )}
      {(dialog === 'sem' || dialog === 'pls') && dataset && (
        <SemBuilder
          key={dialog}
          kind={dialog}
          def={ANALYSES[dialog]}
          dataset={dataset}
          initial={initialArgs ? restore(ANALYSES[dialog], initialArgs) : undefined}
          initialArgs={initialArgs}
          onPaste={paste}
          onClose={() => setDialog(null)}
          onDone={(items) => {
            setOutput((o) => [...o, ...items]);
            setDialog(null);
            setScreen('output');
            refresh();
          }}
        />
      )}
      {dialog && ANALYSES[dialog] && dialog !== 'sem' && dialog !== 'pls' && dataset && (
        <AnalysisDialog
          key={dialog}
          def={ANALYSES[dialog]}
          dataset={dataset}
          initial={initialArgs ? restore(ANALYSES[dialog], initialArgs) : undefined}
          onPaste={paste}
          onClose={() => setDialog(null)}
          onDone={(items) => {
            setOutput((o) => [...o, ...items]);
            setDialog(null);
            setScreen('output');
            refresh();
          }}
        />
      )}
      {sheetChoice && (
        <SheetDialog
          fileName={sheetChoice.fileName}
          sheets={sheetChoice.sheets}
          onClose={() => setSheetChoice(null)}
          onPick={(sheet) => {
            const pick = sheetChoice.pick;
            setSheetChoice(null);
            pick(sheet);
          }}
        />
      )}
      {dialog === 'ai' && <AiSettingsDialog onClose={() => setDialog(null)} onChange={setAi} />}
      {dialog === 'mcp' && <AssistantDialog onClose={() => setDialog(null)} />}
      {dialog === 'examples' && <ExamplesDialog onClose={() => setDialog(null)} onOpen={openExample} onWorked={() => (setDialog(null), setHelp('two-groups'))} />}
      {dialog === 'settings' && <SettingsDialog onClose={() => setDialog(null)} onAi={() => setDialog('ai')} onAssistant={() => setDialog('mcp')} />}
      {dialog === 'design' && (
        <DesignDialog
          onClose={() => setDialog(null)}
          onCreate={async (spec) => {
            if (!(await okToReplace())) return;
            const r = await call<{ report: ImportReport; dataset: DatasetInfo; state: ProjectState; outputs: OutputItem[] }>('create_design', spec);
            setDialog(null);
            opened(r, r.dataset.name);
            setOutput(r.outputs);
            setScreen('output');
          }}
        />
      )}
      {dialog === 'power' && (
        <PowerDialog
          initial={initialArgs ?? undefined}
          onPaste={paste}
          onClose={() => setDialog(null)}
          onDone={(items) => {
            setOutput((o) => [...o, ...items]);
            setDialog(null);
            setScreen('output');
            refresh();
          }}
        />
      )}
      {googleOpen && <GoogleSheetDialog onOpen={loadGoogle} onClose={() => setGoogleOpen(false)} />}
      {pathPrompt && (
        <PathDialog
          title={pathPrompt.mode === 'open' ? 'Open project' : 'Save project'}
          action={pathPrompt.mode === 'open' ? 'Open' : 'Save'}
          initial={project?.path ?? ''}
          onClose={() => (pathPrompt.resolve(null), setPathPrompt(null))}
          onSubmit={(p) => (pathPrompt.resolve(p), setPathPrompt(null))}
        />
      )}
      {unsaved && (
        <UnsavedDialog
          onClose={() => (unsaved.resolve(false), setUnsaved(null))}
          onDiscard={() => (unsaved.resolve(true), setUnsaved(null))}
          onSave={() => ((unsaved.resolve as (v: boolean | 'save') => void)('save'), setUnsaved(null))}
        />
      )}
      {dialog === 'about' && (
        <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && setDialog(null)}>
          <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="aboutTitle" style={{ width: 'min(460px,100%)' }}>
            <header>
              <h2 id="aboutTitle">About PrismStatistics</h2>
              <button type="button" className="iconbtn x" aria-label="Close" onClick={() => setDialog(null)} autoFocus>
                <Icon name="close" />
              </button>
            </header>
            <div className="dbody">
              <p style={{ margin: 0 }}>Version {__APP_VERSION__}, teaching preview. Part of the PrismOpenResearch suite. Statistics are shared with PrismQDA and checked against R.</p>
              <p className="muted" style={{ margin: 0 }}>Apache License 2.0.</p>
            </div>
          </div>
        </div>
      )}
      <div className={`toast ${toast ? 'show' : ''}`} role="status" aria-live="polite">
        {toast}
      </div>
      {help !== undefined && <HelpDialog key={help ?? ''} start={help} onClose={() => setHelp(undefined)} onExample={openExample} />}
    </div>
    </HelpContext.Provider>
  );
}
