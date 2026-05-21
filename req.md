
## 1. Módulo de Análise Global (Visão Macro-Mercado)

Focado em consolidar as carteiras coletadas para oferecer ao usuário ferramentas de filtragem e cruzamento de dados.

### Requisitos Funcionais (RF)

* **RF03 - Mapeamento de Concentração de Ativos:** Consolidar a base de dados para identificar quais papéis (ações/tickers) são os mais recorrentes entre todos os fundos monitorados.
* **RF04 - Volume Financeiro Agregado por Papel:** Calcular o montante financeiro total (somatório em R$) alocado por todos os fundos da base em um determinado ativo dentro de um recorte temporal específico.
* **RF05 - Análise de Extremos Nominais (R$):** Identificar de forma automatizada quais fundos detêm a maior e a menor exposição em valores absolutos (R$) para um papel selecionado.
* **RF06 - Análise de Extremos Percentuais (%):** Identificar qual fundo possui a maior e a menor exposição proporcional ($\%$) de seu patrimônio líquido alocada em um mesmo papel.

---

## 2. Módulo de Análise Individual e Data Mining (Comportamento do Gestor)

Módulo analítico avançado focado na identificação de padrões de comportamento, gatilhos de negociação e estimativas matemáticas de posições ocultas.

### Requisitos Funcionais (RF)

* **RF07 - Mineração de Gatilho de Desinvestimento (Take-Profit):** Analisar a série histórica para detectar o limite de risco do gestor. O sistema calculará a partir de qual percentual ($\%$) máximo de exposição sobre o patrimônio o fundo inicia vendas sistemáticas para diminuir o peso do papel na carteira.
* **RF08 - Cálculo de Tempo de Inércia de Posição:** Identificar se o movimento de "montar" ou "desfazer" uma posição relevante em um papel ocorre de forma abrupta (dentro do mesmo mês fiscal) ou se dilui em uma janela temporal estendida (janela superior a 1 mês).
* **RF09 - Estimativa de Preço Médio de Compra:** Calcular o valor médio de aquisição de um ativo assim que ele passa a figurar no histórico do fundo, utilizando a variação de volume financeiro e quantidade:

$$Preço\ Médio_{Compra} = \frac{\Delta\ Valor\ Aplicado_{Compra}}{\Delta\ Quantidade_{Compra}}$$

* **RF10 - Estimativa de Preço Médio de Venda:** Calcular o valor médio de saída ou redução de posição do ativo, acompanhando os meses em que há decréscimo na quantidade de papéis retidos em carteira.
* **RF11 - Extrapolação de Posições Recentes (Ocultas):** Desenvolver algoritmo preditivo/extrapolador para os meses em que a B3/CVM aplica a regra de confidencialidade (escondendo os detalhes da carteira nos meses mais recentes). Utilizando apenas os dados consolidados informados em Reais (R$), o valor da cotação histórica do papel no encerramento de cada mês e o comportamento anterior do fundo, o sistema deve extrapolar a quantidade provável de papéis que o fundo está comprado no período oculto.
