#!/usr/bin/env python3
"""Gráfico Definitivo de Validação Metrológica — Auditoria do Filtro de Kalman.

Gera o gráfico de validação com:
  - Linha real (real_qty) com marcadores no passado
  - Linha do filtro (filtered_qty) contínua
  - Área sombreada ci_lower / ci_upper (cone IC95)
  - Pintura de fundo: passado (is_future=0) vs futuro (is_future=1)
  - Métricas de acurácia direcional e cobertura no título

Modos de uso:
  python plot_definitive_validation.py                          # real_cvm_trajectories.csv
  python plot_definitive_validation.py results/                  # todos results/**/metrology.csv
  python plot_definitive_validation.py results/FUNDO/ATIVO/metrology.csv  # único
"""
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.dates as mdates
import numpy as np
import sys
import os
import glob

# ── Config ──────────────────────────────────────────────────────────────────
plt.style.use('default')
plt.rcParams.update({
    'font.family': 'serif',
    'font.size': 11,
    'axes.titlesize': 12,
    'axes.labelsize': 11,
    'figure.dpi': 150,
})

# ── Colors ──────────────────────────────────────────────────────────────────
PAST_BG = '#f0f4ff'
FUTURE_BG = '#fff5f5'
REAL_COLOR = '#1a1a2e'
FILTER_COLOR = '#2563eb'
FUTURE_LINE = '#dc2626'
CI_PAST = '#93c5fd'
CI_FUTURE = '#fca5a5'
DIVIDER_COLOR = '#94a3b8'
OUT_DIR = 'definitive_validation_plots'


def process_single(csv_path, fund_id=None, asset=None, out_subdir=None):
    """Processa um arquivo metrology.csv e gera o gráfico definitivo."""
    df = pd.read_csv(csv_path)

    # Normalizar colunas
    if 't_dt' in df.columns and 't' not in df.columns:
        df = df.rename(columns={'t_dt': 't'})

    # Extrair fund_id/asset do CSV se não fornecidos
    if fund_id is None and 'fund_id' in df.columns:
        fund_id = str(df['fund_id'].iloc[0])
    if asset is None and 'asset' in df.columns:
        asset = str(df['asset'].iloc[0])

    # Fallback do caminho
    if fund_id is None or asset is None:
        parts = csv_path.replace('\\', '/').split('/')
        if len(parts) >= 2:
            asset = parts[-2] if asset is None else asset
        if len(parts) >= 3:
            fund_id = parts[-3] if fund_id is None else fund_id

    fund_id = str(fund_id or 'UNKNOWN')
    asset = str(asset or 'UNKNOWN')
    safe_fund = fund_id.replace('/', '_').replace('.', '').replace('-', '')

    # Verificar se já existe o par (para monolithic CSV)
    if 'fund_id' in df.columns and df['fund_id'].nunique() > 1 and out_subdir is None:
        return  # será processado pelo caller que extrai pares

    # Se o CSV tem múltiplos pares, extrair só este
    if 'fund_id' in df.columns and 'asset' in df.columns:
        mask = (df['fund_id'].astype(str) == fund_id) & (df['asset'].astype(str) == asset)
        d = df[mask].copy()
    else:
        d = df.copy()

    if len(d) < 3:
        print(f"  ⚠ {fund_id}/{asset}: dados insuficientes ({len(d)} linhas)")
        return

    # Detectar se 't' é data ou índice
    t_is_date = False
    sample = str(d['t'].iloc[0])
    # Tenta parsing de data em vários formatos (incluindo mistos na mesma coluna)
    try:
        if sample.count('-') >= 2 or sample.count('/') >= 2:
            d['t_parsed'] = pd.to_datetime(d['t'])
        elif len(sample) == 7 and sample.count('-') == 1:
            # Coluna pode ter mistura de 'YYYY-MM' e 'YYYY-MM-DD'
            t_str = d['t'].astype(str)
            # Valores com len 7 (YYYY-MM) ganham '-01'; os demais já têm dia
            fixed = t_str.where(t_str.str.len() != 7, t_str + '-01')
            d['t_parsed'] = pd.to_datetime(fixed)
        else:
            raise ValueError("not a date")
        d = d.sort_values('t_parsed')
        t_is_date = True
    except (ValueError, TypeError):
        d = d.sort_values('t')

    past = d[d['is_future'] == 0]
    future = d[d['is_future'] == 1]

    if t_is_date:
        t_all = d['t_parsed'].values
        t_past = past['t_parsed'].values if len(past) > 0 else []
        t_future = future['t_parsed'].values if len(future) > 0 else []
    else:
        t_all = d['t'].values.astype(float)
        t_past = past['t'].values.astype(float) if len(past) > 0 else []
        t_future = future['t'].values.astype(float) if len(future) > 0 else []

    n_past = len(past)
    n_future = len(future)

    # ── Métricas ────────────────────────────────────────────────────────────
    coverage_in = 0
    if n_past > 0:
        ci_mask = (past['real_qty'] >= past['ci_lower']) & (past['real_qty'] <= past['ci_upper'])
        coverage_in = ci_mask.sum()

    directional_hits = 0
    directional_total = 0
    if n_past >= 2:
        past_idx = past.reset_index(drop=True)
        for i in range(1, n_past):
            prev_z = past_idx['z_score'].iloc[i - 1]
            actual_delta = past_idx['real_qty'].iloc[i] - past_idx['real_qty'].iloc[i - 1]
            if abs(actual_delta) < 1e-9:
                continue
            predicted_dir = 1 if prev_z > 2.0 else (-1 if prev_z < -2.0 else 0)
            if predicted_dir == 0:
                continue
            directional_total += 1
            if predicted_dir == (1 if actual_delta > 0 else -1):
                directional_hits += 1

    overall_dir = "—"
    if n_past >= 2:
        delta = past['real_qty'].iloc[-1] - past['real_qty'].iloc[0]
        overall_dir = "↑ Acumulou" if delta > 0 else ("↓ Distribuiu" if delta < 0 else "→ Estável")

    # ── Figura ──────────────────────────────────────────────────────────────
    fig, ax = plt.subplots(figsize=(14, 6))

    # Fundo colorido por regime
    if n_past > 0 and n_future > 0:
        if t_is_date:
            t_div = t_past[-1] + (t_future[0] - t_past[-1]) / 2
        else:
            t_div = t_past[-1] + 0.5
        ax.axvspan(t_all[0] - (pd.Timedelta(days=1) if t_is_date else 0.5),
                   t_div, facecolor=PAST_BG, zorder=0, alpha=0.6)
        ax.axvspan(t_div,
                   t_all[-1] + (pd.Timedelta(days=1) if t_is_date else 0.5),
                   facecolor=FUTURE_BG, zorder=0, alpha=0.6)
        ax.axvline(t_div, color=DIVIDER_COLOR, linestyle='--', linewidth=1.2,
                   label='Passado → Futuro')
    elif n_past > 0:
        ax.axvspan(t_all[0] - (pd.Timedelta(days=1) if t_is_date else 0.5),
                   t_all[-1] + (pd.Timedelta(days=1) if t_is_date else 0.5),
                   facecolor=PAST_BG, zorder=0, alpha=0.4)

    # IC band — passado
    if n_past > 0:
        ax.fill_between(t_past, past['ci_lower'], past['ci_upper'],
                        color=CI_PAST, alpha=0.4, zorder=1, label='IC 95% (Passado)')
    # IC band — futuro
    if n_future > 0:
        ax.fill_between(t_future, future['ci_lower'], future['ci_upper'],
                        color=CI_FUTURE, alpha=0.35, zorder=1, label='IC 95% (Futuro)')

    # Linha real
    if n_past > 0:
        ax.plot(t_past, past['real_qty'].values, 'o', color=REAL_COLOR, markersize=4.5,
                zorder=3, label='$Q_t$ Real (CVM)')
    # Linha filtrada — passado
    if n_past > 0:
        ax.plot(t_past, past['filtered_qty'].values, '-', color=FILTER_COLOR, linewidth=2.0,
                zorder=2, label='$\\hat{Q}_t$ Filtro Kalman')
    # Linha projetada — futuro
    if n_future > 0:
        ax.plot(t_future, future['filtered_qty'].values, '--', color=FUTURE_LINE, linewidth=2.2,
                zorder=2, label='$\\tilde{Q}_{t+k}$ Projeção')

    # ── Título com métricas ─────────────────────────────────────────────────
    cov_pct = 100 * coverage_in / max(n_past, 1)
    dir_acc_pct = 100 * directional_hits / max(directional_total, 1) if directional_total > 0 else 0
    title_lines = [
        "Validação Metrológica — Filtro de Kalman 2D (posição-velocidade)",
        f"Fundo: {fund_id}  |  Ativo: {asset}",
        f"Cobertura IC95: {coverage_in}/{n_past} = {cov_pct:.0f}%  |  "
        f"Acurácia Direcional: {directional_hits}/{directional_total} ({dir_acc_pct:.0f}%)  |  "
        f"Tendência Global: {overall_dir}",
    ]
    ax.set_title('\n'.join(title_lines), fontsize=11, fontweight='bold', loc='center', pad=18)

    if t_is_date:
        ax.xaxis.set_major_formatter(mdates.DateFormatter('%Y-%m'))
        ax.xaxis.set_major_locator(mdates.MonthLocator(interval=max(1, n_past // 8)))
        plt.setp(ax.xaxis.get_majorticklabels(), rotation=45, ha='right')
        ax.set_xlabel('Tempo')
    else:
        ax.set_xlabel('Tempo (índice mensal)')

    ax.set_ylabel('Quantidade de Cotas')
    ax.legend(loc='upper left', framealpha=0.9, fontsize=9)
    ax.grid(True, alpha=0.25, linestyle=':')

    # Limites Y
    all_vals = np.concatenate([
        past['real_qty'].values if n_past > 0 else [],
        past['ci_lower'].values if n_past > 0 else [],
        past['ci_upper'].values if n_past > 0 else [],
        future['ci_lower'].values if n_future > 0 else [],
        future['ci_upper'].values if n_future > 0 else [],
    ])
    if len(all_vals) > 0:
        y_min, y_max = np.min(all_vals), np.max(all_vals)
        pad = (y_max - y_min) * 0.08 if y_max > y_min else 1.0
        ax.set_ylim(max(0, y_min - pad), y_max + pad)

    # Legendas de regime
    if n_past > 0:
        ax.annotate('REGIME DE\nOBSERVAÇÃO\n(is_future=0)',
                    xy=(0.02, 0.92), xycoords='axes fraction',
                    fontsize=8, color='#1e40af', fontweight='bold', ha='left', va='top',
                    bbox=dict(boxstyle='round,pad=0.4', facecolor=PAST_BG,
                              edgecolor='#bfdbfe', alpha=0.9))
    if n_future > 0:
        ax.annotate('REGIME DE\nPREVISÃO\n(is_future=1)',
                    xy=(0.98, 0.92), xycoords='axes fraction',
                    fontsize=8, color='#991b1b', fontweight='bold', ha='right', va='top',
                    bbox=dict(boxstyle='round,pad=0.4', facecolor=FUTURE_BG,
                              edgecolor='#fecaca', alpha=0.9))

    plt.tight_layout()

    # Output — salva no subdiretório do ativo se veio de results/
    if out_subdir:
        os.makedirs(out_subdir, exist_ok=True)
        out_path = os.path.join(out_subdir, f'definitive_{safe_fund}_{asset}.png')
    else:
        os.makedirs(OUT_DIR, exist_ok=True)
        out_path = os.path.join(OUT_DIR, f'definitive_{safe_fund}_{asset}.png')

    plt.savefig(out_path, dpi=300, bbox_inches='tight')
    plt.close(fig)
    return (coverage_in, n_past, directional_hits, directional_total, out_path)


# ═══════════════════════════════════════════════════════════════════════════════
# Main
# ═══════════════════════════════════════════════════════════════════════════════
if __name__ == '__main__':
    arg = sys.argv[1] if len(sys.argv) > 1 else 'real_cvm_trajectories.csv'

    if os.path.isdir(arg):
        # ── Modo diretório: vasculha results/**/metrology.csv ─────────────
        csv_files = glob.glob(os.path.join(arg, '**', 'metrology.csv'), recursive=True)
        if not csv_files:
            print(f"Nenhum metrology.csv encontrado em '{arg}/'")
            sys.exit(1)
        print(f"Encontrados {len(csv_files)} arquivos metrology.csv em '{arg}/'\n")

        grand_cov_in, grand_cov_tot = 0, 0
        grand_dir_hits, grand_dir_tot = 0, 0
        for csv_path in sorted(csv_files):
            # Extrai fundo/ativo do caminho: results/FUNDO/ATIVO/metrology.csv
            parts = csv_path.replace('\\', '/').split('/')
            asset_name = parts[-2] if len(parts) >= 2 else 'UNKNOWN'
            fund_name = parts[-3] if len(parts) >= 3 else 'UNKNOWN'
            out_sub = os.path.dirname(csv_path)

            result = process_single(csv_path, fund_id=fund_name, asset=asset_name, out_subdir=out_sub)
            if result:
                cov_in, cov_tot, dir_hits, dir_tot, out_path = result
                grand_cov_in += cov_in
                grand_cov_tot += cov_tot
                grand_dir_hits += dir_hits
                grand_dir_tot += dir_tot
                cov_pct = 100 * cov_in / max(cov_tot, 1)
                dir_pct = 100 * dir_hits / max(dir_tot, 1) if dir_tot > 0 else 0
                print(f"  ✓ {out_path}  |  IC95: {cov_in}/{cov_tot}={cov_pct:.0f}%  |  Dir: {dir_hits}/{dir_tot}={dir_pct:.0f}%")

        print(f"\n── Agregado sobre {len(csv_files)} pares fundo/ativo ──")
        if grand_cov_tot > 0:
            print(f"  Cobertura IC95 total: {grand_cov_in}/{grand_cov_tot} = {100*grand_cov_in/grand_cov_tot:.1f}%")
        if grand_dir_tot > 0:
            print(f"  Acurácia Direcional total: {grand_dir_hits}/{grand_dir_tot} = {100*grand_dir_hits/grand_dir_tot:.1f}%")
        print("  Pronto.")

    elif os.path.isfile(arg):
        # ── Modo arquivo único (monolítico ou individual) ─────────────────
        df = pd.read_csv(arg)

        # Normalizar coluna de tempo
        if 't_dt' in df.columns and 't' not in df.columns:
            df = df.rename(columns={'t_dt': 't'})

        # Determinar se é monolítico (múltiplos pares) ou individual
        has_pairs = 'fund_id' in df.columns and 'asset' in df.columns and df[['fund_id', 'asset']].drop_duplicates().shape[0] > 1

        if has_pairs:
            # Monolítico: priorizar ativos famosos
            famous = ['HAPV3', 'GGPS3', 'POMO4', 'UGPA3', 'RECV3',
                      'RENT3', 'EQTL3', 'VIVT3', 'GGBR4', 'SBFG3', 'STBP3',
                      'VALE3', 'PETR4', 'ITUB4', 'BBDC4', 'ABEV3', 'WEGE3']
            pairs = df[df['asset'].isin(famous)][['fund_id', 'asset']].drop_duplicates().values.tolist()
            if not pairs:
                pairs = df[['fund_id', 'asset']].drop_duplicates().values.tolist()

            print(f"Processando {len(pairs)} pares fundo/ativo de '{arg}'\n")
            for fund_id, asset in pairs:
                result = process_single(arg, fund_id=str(fund_id), asset=str(asset))
                if result:
                    cov_in, cov_tot, dir_hits, dir_tot, out_path = result
                    cov_pct = 100 * cov_in / max(cov_tot, 1)
                    dir_pct = 100 * dir_hits / max(dir_tot, 1) if dir_tot > 0 else 0
                    print(f"  ✓ {out_path}  |  IC95: {cov_in}/{cov_tot}={cov_pct:.0f}%  |  Dir: {dir_hits}/{dir_tot}={dir_pct:.0f}%")
            print(f"\n{len(pairs)} gráficos gerados em '{OUT_DIR}/'.")
        else:
            # Arquivo individual (ex: results/FUNDO/ATIVO/metrology.csv)
            parts = arg.replace('\\', '/').split('/')
            asset_name = parts[-2] if len(parts) >= 2 else None
            fund_name = parts[-3] if len(parts) >= 3 else None
            out_sub = os.path.dirname(arg) if fund_name else None
            result = process_single(arg, fund_id=fund_name, asset=asset_name, out_subdir=out_sub)
            if result:
                cov_in, cov_tot, dir_hits, dir_tot, out_path = result
                cov_pct = 100 * cov_in / max(cov_tot, 1)
                dir_pct = 100 * dir_hits / max(dir_tot, 1) if dir_tot > 0 else 0
                print(f"✓ {out_path}  |  IC95: {cov_in}/{cov_tot}={cov_pct:.0f}%  |  Dir: {dir_hits}/{dir_tot}={dir_pct:.0f}%")
            else:
                print(f"Dados insuficientes em '{arg}'")
    else:
        print(f"Erro: '{arg}' não é um arquivo nem diretório.")
        print("Uso: python plot_definitive_validation.py [results/ | metrology.csv | real_cvm_trajectories.csv]")
        sys.exit(1)
