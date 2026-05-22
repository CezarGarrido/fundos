import pandas as pd
import matplotlib.pyplot as plt
import numpy as np

# Ler os dados da série temporal gerada
df = pd.read_csv('/home/carrefour/Dev/Rust/fundos/academic_charts_data.csv')

# Estilo acadêmico e paleta de cores moderna (fundo branco para artigos)
plt.style.use('default')
plt.rcParams['font.family'] = 'serif'

past = df[df['is_future'] == 0]
future = df[df['is_future'] == 1]

################################################################
# 1. Gráfico de "Cone de Incerteza" (Probabilistic Forecast)
################################################################
fig, ax = plt.subplots(figsize=(10, 5))

# Plot Histórico (Kalman Filtered vs Real)
ax.plot(past['t'], past['real_qty'], 'k.', alpha=0.5, label='Sinal Ruidoso (CVM)')
ax.plot(past['t'], past['filtered_qty'], 'b-', linewidth=2, label='Filtro de Kalman ($\hat{Q}_t$)')

# Plot Futuro (Cone colapsando/expandindo)
ax.plot(future['t'], future['filtered_qty'], 'b--', linewidth=2, label='Previsão')

# Preenchimento do IC95 com degradê
# Para simplificar o degradê em plt.fill_between, usaremos transparência simples (alpha)
ax.fill_between(past['t'], past['ci_lower'], past['ci_upper'], color='blue', alpha=0.1)
ax.fill_between(future['t'], future['ci_lower'], future['ci_upper'], color='red', alpha=0.15, label='Cone $IC_{95}$ (Alarga no futuro)')

# Marca a quebra de regime t=30
ax.axvline(x=29.5, color='gray', linestyle='--')
ax.text(30, df['real_qty'].max()*0.9, ' Ponto de Cegueira', color='gray')

ax.set_title('1. Cone de Incerteza e Filtro de Estado Latente')
ax.set_ylabel('Quantidade de Cotas')
ax.set_xlabel('Tempo (Meses)')
ax.legend()
plt.tight_layout()
plt.savefig('fig1_cone_incerteza.png', dpi=300)
print("Gerado: fig1_cone_incerteza.png")


################################################################
# 2. Gráfico de "Resíduos de Inovação" (Validation Diagnostic)
################################################################
fig, ax = plt.subplots(figsize=(10, 5))

# Plotamos os resíduos apenas da parte filtrada (passado) para ver a calibração
ax.stem(past['t'], past['residual'], linefmt='grey', markerfmt='ko', basefmt='k-')

# Ruído branco esperado: linha no zero
ax.axhline(0, color='red', linewidth=1)

ax.set_title('2. Diagnóstico de Inovação (Ruído Branco nos Resíduos)')
ax.set_ylabel('Resíduo ($y_t - \hat{y}_t$)')
ax.set_xlabel('Tempo (Meses)')
plt.tight_layout()
plt.savefig('fig2_residuos_inovacao.png', dpi=300)
print("Gerado: fig2_residuos_inovacao.png")


################################################################
# 3. Gráfico de "Convergência do Z-Score" (Drift Detection)
################################################################
fig, ax = plt.subplots(figsize=(10, 5))

# Z-score line
ax.plot(past['t'], past['z_score'], 'g-', linewidth=2, label='Evolução do Z-Score ($\mu_t / \sigma_\mu$)')

# Linhas de significância estrita
ax.axhline(2.0, color='red', linestyle='--', label='Acumulando Significativo (+2.0)')
ax.axhline(-2.0, color='red', linestyle='--', label='Distribuindo Significativo (-2.0)')
ax.fill_between(past['t'], -2.0, 2.0, color='gray', alpha=0.1, label='Zona Neutra (Consistente)')

ax.set_title('3. Detecção Direcional de Mudança de Regime (Z-Score)')
ax.set_ylabel('Estatística Z')
ax.set_xlabel('Tempo (Meses)')
ax.legend(loc='upper left')
plt.tight_layout()
plt.savefig('fig3_zscore_drift.png', dpi=300)
print("Gerado: fig3_zscore_drift.png")


################################################################
# 4. Gráfico de "Backtest de Cobertura" (Pillar 1 Analysis)
################################################################
# Aqui plotamos apenas a projeção Walk-Forward futura para provar cobertura
fig, ax = plt.subplots(figsize=(10, 5))

# Cone de incerteza da predição
ax.fill_between(future['t'], future['ci_lower'], future['ci_upper'], color='blue', alpha=0.15, label='Banda de Confiança 95%')
ax.plot(future['t'], future['filtered_qty'], 'b-', label='Projeção Teórica')

# Pontos observados no mundo real
outliers_idx = []
inliers_idx = []
for idx, row in future.iterrows():
    if row['real_qty'] > row['ci_upper'] or row['real_qty'] < row['ci_lower']:
        outliers_idx.append(idx)
    else:
        inliers_idx.append(idx)

ax.scatter(future.loc[inliers_idx, 't'], future.loc[inliers_idx, 'real_qty'], color='green', marker='o', s=100, label='Inliers (Realidade Coberta)')
ax.scatter(future.loc[outliers_idx, 't'], future.loc[outliers_idx, 'real_qty'], color='red', marker='x', s=100, label='Outliers (Violação do Cone)')

ax.set_title('4. Backtest de Cobertura Cross-Sectional no Futuro')
ax.set_ylabel('Volume Físico')
ax.set_xlabel('Tempo (Meses)')
ax.legend()
plt.tight_layout()
plt.savefig('fig4_backtest_cobertura.png', dpi=300)
print("Gerado: fig4_backtest_cobertura.png")
