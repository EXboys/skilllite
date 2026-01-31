---
name: data-analysis
description: 提供全面的数据清洗和统计分析功能。支持处理缺失值、去除重复值、数据类型转换、异常值处理、字符串清理等数据清洗操作，以及描述性统计、相关性分析、分组统计、假设检验、数据分布分析等统计功能。当用户需要对数据进行清洗预处理或统计分析时使用。
---

# 数据分析技能

这个技能提供了一套完整的数据清洗和统计分析工具，帮助用户高效地处理和分析数据。

## 功能概述

### 数据清洗功能
- **处理缺失值**: 支持删除、填充均值/中位数/众数/零值等多种策略
- **去除重复值**: 识别并删除重复的行或记录
- **数据类型转换**: 将列转换为指定的数据类型（数值、日期、字符串等）
- **异常值处理**: 使用IQR或Z-score方法识别和处理异常值
- **字符串清理**: 清理字符串数据（去除空格、大小写转换、去除多余空格等）

### 统计分析功能
- **描述性统计**: 计算均值、中位数、标准差、分位数等基本统计量
- **相关性分析**: 计算变量间的相关系数（Pearson、Spearman、Kendall）
- **分组统计**: 按指定列分组并计算聚合统计量
- **假设检验**: 支持t检验、Mann-Whitney U检验、卡方检验、ANOVA等
- **数据分布分析**: 分析数据分布特征并进行正态性检验

## 使用方法

### 数据清洗

#### 处理缺失值
```python
from scripts.data_cleaning import handle_missing_values

# 删除包含缺失值的行
df_clean = handle_missing_values(df, strategy='drop')

# 用均值填充数值列的缺失值
df_clean = handle_missing_values(df, strategy='fill_mean')

# 用0填充指定列的缺失值
df_clean = handle_missing_values(df, strategy='fill_zero', columns=['age', 'salary'])
```

#### 去除重复值
```python
from scripts.data_cleaning import remove_duplicates

# 去除所有列的重复行
df_clean = remove_duplicates(df)

# 基于指定列去除重复值
df_clean = remove_duplicates(df, subset=['name', 'email'])
```

#### 处理异常值
```python
from scripts.data_cleaning import handle_outliers

# 使用IQR方法处理异常值
df_clean = handle_outliers(df, method='iqr', threshold=1.5)

# 使用Z-score方法处理异常值
df_clean = handle_outliers(df, method='zscore', threshold=3.0)
```

#### 字符串清理
```python
from scripts.data_cleaning import clean_strings

# 执行多种字符串清理操作
df_clean = clean_strings(df, operations=['strip', 'lower', 'remove_extra_spaces'])
```

#### 综合数据清洗
```python
from scripts.data_cleaning import comprehensive_cleaning

# 执行综合数据清洗
df_clean = comprehensive_cleaning(
    df,
    missing_strategy='fill_mean',
    handle_duplicates=True,
    outlier_method='iqr',
    string_operations=['strip', 'lower']
)
```

### 统计分析

#### 描述性统计
```python
from scripts.statistical_analysis import descriptive_statistics

# 计算所有数值列的描述性统计
stats_df = descriptive_statistics(df)

# 计算指定列的描述性统计
stats_df = descriptive_statistics(df, columns=['age', 'salary'])
```

#### 相关性分析
```python
from scripts.statistical_analysis import correlation_analysis

# 计算Pearson相关系数
corr_matrix = correlation_analysis(df, method='pearson')

# 计算Spearman相关系数
corr_matrix = correlation_analysis(df, method='spearman')
```

#### 分组统计
```python
from scripts.statistical_analysis import group_statistics

# 按部门分组计算统计量
group_stats = group_statistics(df, group_by='department')

# 指定聚合函数和列
group_stats = group_statistics(
    df, 
    group_by='department',
    agg_columns=['salary', 'age'],
    agg_funcs=['mean', 'median', 'count']
)
```

#### 假设检验
```python
from scripts.statistical_analysis import hypothesis_test

# 独立样本t检验
result = hypothesis_test(
    df, 
    test_type='ttest_ind',
    group_col='gender',
    value_col='salary'
)

# 卡方检验
result = hypothesis_test(
    df,
    test_type='chi2',
    contingency_table=[[10, 20], [15, 25]]
)
```

#### 数据分布分析
```python
from scripts.statistical_analysis import distribution_analysis

# 分析单个变量的分布
dist_result = distribution_analysis(df, 'salary')

# 检查正态性
is_normal = dist_result['normality_test']['is_normal']
```

#### 综合统计分析
```python
from scripts.statistical_analysis import comprehensive_analysis

# 执行综合统计分析
analysis_result = comprehensive_analysis(df, columns=['age', 'salary'])

# 包含分组分析
analysis_result = comprehensive_analysis(df, columns=['age', 'salary'], group_by='department')
```

## 参数说明

### 数据清洗函数参数

#### handle_missing_values
- `df`: 输入的DataFrame（必需）
- `strategy`: 处理策略，可选值：'drop', 'fill_mean', 'fill_median', 'fill_mode', 'fill_zero'
- `columns`: 指定要处理的列列表，如果为None则处理所有列

#### remove_duplicates
- `df`: 输入的DataFrame（必需）
- `subset`: 用于判断重复的列列表，如果为None则考虑所有列
- `keep`: 保留策略，可选值：'first', 'last', False

#### handle_outliers
- `df`: 输入的DataFrame（必需）
- `columns`: 要处理的数值列列表，如果为None则自动检测数值列
- `method`: 异常值检测方法，可选值：'iqr', 'zscore'
- `threshold`: 阈值，默认为1.5（IQR）或3.0（Z-score）

#### clean_strings
- `df`: 输入的DataFrame（必需）
- `columns`: 要清理的字符串列列表，如果为None则自动检测字符串列
- `operations`: 操作列表，可选值：'strip', 'lower', 'upper', 'remove_extra_spaces'

### 统计分析函数参数

#### descriptive_statistics
- `df`: 输入的DataFrame（必需）
- `columns`: 要分析的列列表，如果为None则分析所有数值列

#### correlation_analysis
- `df`: 输入的DataFrame（必需）
- `columns`: 要分析的列列表，如果为None则分析所有数值列
- `method`: 相关性计算方法，可选值：'pearson', 'spearman', 'kendall'

#### group_statistics
- `df`: 输入的DataFrame（必需）
- `group_by`: 分组列名（必需）
- `agg_columns`: 要聚合的列列表，如果为None则使用所有数值列
- `agg_funcs`: 聚合函数列表，默认为['mean', 'count', 'sum']

#### hypothesis_test
- `df`: 输入的DataFrame（必需）
- `test_type`: 检验类型，可选值：'ttest_ind', 'ttest_rel', 'mannwhitneyu', 'chi2', 'anova'
- 其他参数根据检验类型而定

#### distribution_analysis
- `df`: 输入的DataFrame（必需）
- `column`: 要分析的列名（必需）
- `test_normality`: 是否进行正态性检验，默认为True

## 注意事项

1. **数据类型**: 确保输入数据是pandas DataFrame格式
2. **内存使用**: 对于大型数据集，某些操作可能会消耗大量内存
3. **缺失值处理**: 在进行统计分析前，建议先处理缺失值
4. **异常值影响**: 异常值可能会影响统计结果，建议先进行异常值检测
5. **假设检验前提**: 进行假设检验前，请确保满足检验的前提条件

## 依赖库

- pandas >= 2.0.0
- numpy >= 1.24.0
- scipy >= 1.10.0

## 示例数据

技能包中包含示例数据文件 `test_data.csv`，可用于测试各项功能。