import pandas as pd
import matplotlib.pyplot as plt
import os

# Load the real trajectories
df = pd.read_csv('/home/carrefour/Dev/Rust/fundos/real_cvm_trajectories.csv')

# Prefer to plot famous stocks that were downloaded
famous = ['RENT3', 'EQTL3', 'VIVT3', 'GGBR4', 'SBFG3', 'STBP3']

# Find all unique combinations of (fund_id, asset) where asset is famous
df_famous = df[df['asset'].isin(famous)]
pairs = df_famous[['fund_id', 'asset']].drop_duplicates().values.tolist()

if not pairs:
    pairs = df[['fund_id', 'asset']].drop_duplicates().values.tolist()

fund_to_plot, asset_to_plot = pairs[0]
print(f"Plotando projeção para Fundo: {fund_to_plot} | Ativo: {asset_to_plot}")

# Filter data for this specific fund and asset
df_asset = df[(df['fund_id'] == fund_to_plot) & (df['asset'] == asset_to_plot)].sort_values(by='t')

# Set up academic style plot
plt.style.use('default')
plt.rcParams['font.family'] = 'serif'

fig, ax = plt.subplots(figsize=(10, 5))

# Plot the projection (is_future = 1)
ax.plot(df_asset['t'], df_asset['filtered_qty'], 'b--', linewidth=2, label=f'Previsão ({asset_to_plot})')

# Fill the uncertainty cone
ax.fill_between(df_asset['t'], df_asset['ci_lower'], df_asset['ci_upper'], color='red', alpha=0.15, label='Cone $IC_{95}$ (Incerteza Futura)')

ax.set_title(f'Projeção Walk-Forward - Fundo {fund_to_plot} | Ativo {asset_to_plot}')
ax.set_ylabel('Quantidade Estimada')
ax.set_xlabel('Tempo Projetado (Meses a frente)')
ax.legend()
plt.tight_layout()

# Save with a safe filename
safe_fund_id = fund_to_plot.replace('/', '_').replace('.', '').replace('-', '')
output_file = f'fig_real_projection_{safe_fund_id}_{asset_to_plot}.png'
plt.savefig(output_file, dpi=300)
print(f"Gerado: {output_file}")

