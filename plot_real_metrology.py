import pandas as pd
import matplotlib.pyplot as plt
import sys
import os

csv_path = sys.argv[1] if len(sys.argv) > 1 else '/home/carrefour/Dev/Rust/fundos/real_cvm_trajectories.csv'

# Ler os dados reais completos da série temporal gerada
df = pd.read_csv(csv_path)

if len(sys.argv) > 1:
    # MODO ARQUIVO ÚNICO (ex: results/CNPJ/ATIVO/metrology.csv)
    fund_to_plot = str(df['fund_id'].iloc[0])
    asset_to_plot = str(df['asset'].iloc[0])
    pairs = [(fund_to_plot, asset_to_plot)]
    base_dir = os.path.dirname(csv_path) # Salva no próprio diretório
else:
    # Filtrar para encontrar um par com cotações válidas
    famous = ['HAPV3', 'GGPS3', 'POMO4', 'UGPA3', 'RECV3', 'RENT3', 'EQTL3', 'VIVT3', 'GGBR4', 'SBFG3', 'STBP3']
    df_famous = df[df['asset'].isin(famous)]
    pairs = df_famous[['fund_id', 'asset']].drop_duplicates().values.tolist()

    if not pairs:
        pairs = df[['fund_id', 'asset']].drop_duplicates().values.tolist()

    base_dir = 'metrology_plots'
    os.makedirs(base_dir, exist_ok=True)

for fund_to_plot, asset_to_plot in pairs:
    print(f"Gerando METROLOGIA COMPLETA para Fundo: {fund_to_plot} | Ativo: {asset_to_plot}")

    safe_fund_id = str(fund_to_plot).replace('/', '_').replace('.', '').replace('-', '')
    
    if len(sys.argv) > 1:
        asset_dir = base_dir # Save directly where the CSV is
    else:
        fund_dir = os.path.join(base_dir, safe_fund_id)
        os.makedirs(fund_dir, exist_ok=True)
        asset_dir = os.path.join(fund_dir, asset_to_plot)
        os.makedirs(asset_dir, exist_ok=True)

    # Handle time column name differences
    if 't' not in df.columns and 't_dt' in df.columns:
        df = df.rename(columns={'t_dt': 't'})

    # Filtrar os dados desse par específico e ordenar no tempo
    if str(df['fund_id'].dtype) != 'object':
        df['fund_id'] = df['fund_id'].astype(str)
    
    df_asset = df[(df['fund_id'] == fund_to_plot) & (df['asset'] == asset_to_plot)].sort_values(by='t').copy()

    past = df_asset[df_asset['is_future'] == 0]
    future = df_asset[df_asset['is_future'] == 1]

    # Estilo acadêmico e paleta de cores moderna (fundo branco para artigos)
    plt.style.use('default')
    plt.rcParams['font.family'] = 'serif'

    prefix = f'real_metrology_{safe_fund_id}_{asset_to_plot}'

    ################################################################
    # 1. Gráfico de "Cone de Incerteza" (Probabilistic Forecast)
    ################################################################
    fig, ax = plt.subplots(figsize=(10, 5))

    # Plot Histórico (Kalman Filtered vs Real)
    ax.plot(past['t'], past['real_qty'], 'k.', alpha=0.5, label='Real Observado')
    ax.plot(past['t'], past['filtered_qty'], 'b-', linewidth=2, label='Filtro de Kalman ($\\hat{Q}_t$)')

    # Plot Futuro (Cone)
    ax.plot(future['t'], future['filtered_qty'], 'b--', linewidth=2, label='Previsão Futura')

    # Preenchimento do IC95
    ax.fill_between(past['t'], past['ci_lower'], past['ci_upper'], color='blue', alpha=0.1)
    ax.fill_between(future['t'], future['ci_lower'], future['ci_upper'], color='red', alpha=0.15, label='Cone $IC_{95}$')

    ax.set_title(f'1. Filtro de Estado Latente - Fundo {fund_to_plot} | Ativo {asset_to_plot}')
    ax.set_ylabel('Quantidade Físico/Ajustada')
    ax.set_xlabel('Tempo (Meses)')
    ax.legend()
    plt.tight_layout()
    fig1 = os.path.join(asset_dir, f'fig1_{prefix}_cone.png')
    plt.savefig(fig1, dpi=300)
    plt.close(fig)

    ################################################################
    # 2. Gráfico de "Resíduos de Inovação" (Validation Diagnostic)
    ################################################################
    fig, ax = plt.subplots(figsize=(10, 5))

    ax.stem(past['t'], past['residual'], linefmt='grey', markerfmt='ko', basefmt='k-')
    ax.axhline(0, color='red', linewidth=1)

    ax.set_title(f'2. Diagnóstico de Inovação (Resíduos) - {asset_to_plot}')
    ax.set_ylabel('Resíduo ($y_t - \\hat{y}_t$)')
    ax.set_xlabel('Tempo (Meses Históricos)')
    plt.tight_layout()
    fig2 = os.path.join(asset_dir, f'fig2_{prefix}_residuos.png')
    plt.savefig(fig2, dpi=300)
    plt.close(fig)

    ################################################################
    # 3. Gráfico de "Convergência do Z-Score" (Drift Detection)
    ################################################################
    fig, ax = plt.subplots(figsize=(10, 5))

    ax.plot(past['t'], past['z_score'], 'g-', linewidth=2, label='Evolução do Z-Score')
    ax.axhline(2.0, color='red', linestyle='--', label='Acumulando (+2.0)')
    ax.axhline(-2.0, color='red', linestyle='--', label='Distribuindo (-2.0)')
    ax.fill_between(past['t'], -2.0, 2.0, color='gray', alpha=0.1, label='Zona Neutra')

    ax.set_title(f'3. Detecção de Mudança de Regime (Z-Score) - {asset_to_plot}')
    ax.set_ylabel('Estatística Z')
    ax.set_xlabel('Tempo (Meses Históricos)')
    ax.legend(loc='upper left')
    plt.tight_layout()
    fig3 = os.path.join(asset_dir, f'fig3_{prefix}_zscore.png')
    plt.savefig(fig3, dpi=300)
    plt.close(fig)

    ################################################################
    # 4. Gráfico de "Backtest de Cobertura" (Future only)
    ################################################################
    fig, ax = plt.subplots(figsize=(10, 5))

    ax.fill_between(future['t'], future['ci_lower'], future['ci_upper'], color='blue', alpha=0.15, label='Banda de Confiança 95%')
    ax.plot(future['t'], future['filtered_qty'], 'b-', label='Projeção Teórica')

    ax.set_title(f'4. Projeção Extrapolada Cross-Sectional - {asset_to_plot}')
    ax.set_ylabel('Volume Projetado')
    ax.set_xlabel('Tempo (Meses Futuros)')
    ax.legend()
    plt.tight_layout()
    fig4 = os.path.join(asset_dir, f'fig4_{prefix}_cobertura.png')
    plt.savefig(fig4, dpi=300)
    plt.close(fig)

print(f"Gerados com sucesso todos os {len(pairs) * 4} plots do Metrology, organizados por fundo!")
