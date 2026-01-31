#!/usr/bin/env python3
"""
统计分析功能模块

提供常用的数据统计分析功能，包括：
- 描述性统计
- 相关性分析
- 分组统计
- 假设检验
- 数据分布分析
"""

import pandas as pd
import numpy as np
import scipy.stats as stats
from typing import List, Dict, Any, Optional, Union, Tuple
import warnings
warnings.filterwarnings('ignore')


def descriptive_statistics(df: pd.DataFrame, 
                          columns: Optional[List[str]] = None) -> pd.DataFrame:
    """
    计算描述性统计信息
    
    Args:
        df: 输入的DataFrame
        columns: 指定要分析的列，如果为None则分析所有数值列
        
    Returns:
        包含统计信息的DataFrame
    """
    if columns is None:
        # 只选择数值列
        numeric_cols = df.select_dtypes(include=[np.number]).columns.tolist()
        columns = numeric_cols if numeric_cols else []
    
    if not columns:
        return pd.DataFrame()
    
    stats_dict = {}
    for col in columns:
        if col not in df.columns:
            continue
            
        series = df[col].dropna()
        if len(series) == 0:
            continue
            
        stats_dict[col] = {
            'count': len(series),
            'mean': series.mean(),
            'median': series.median(),
            'std': series.std(),
            'variance': series.var(),
            'min': series.min(),
            'max': series.max(),
            'range': series.max() - series.min(),
            'q25': series.quantile(0.25),
            'q75': series.quantile(0.75),
            'iqr': series.quantile(0.75) - series.quantile(0.25),
            'skewness': series.skew(),
            'kurtosis': series.kurtosis()
        }
    
    return pd.DataFrame(stats_dict).T


def correlation_analysis(df: pd.DataFrame, 
                       columns: Optional[List[str]] = None,
                       method: str = 'pearson') -> pd.DataFrame:
    """
    相关性分析
    
    Args:
        df: 输入的DataFrame
        columns: 指定要分析的列，如果为None则分析所有数值列
        method: 相关性计算方法 ('pearson', 'spearman', 'kendall')
        
    Returns:
        相关性矩阵
    """
    if columns is None:
        columns = df.select_dtypes(include=[np.number]).columns.tolist()
    
    if not columns:
        return pd.DataFrame()
    
    # 过滤存在的列
    valid_columns = [col for col in columns if col in df.columns]
    
    if len(valid_columns) < 2:
        return pd.DataFrame()
    
    return df[valid_columns].corr(method=method)


def group_statistics(df: pd.DataFrame, 
                    group_by: str,
                    agg_columns: Optional[List[str]] = None,
                    agg_funcs: List[str] = ['mean', 'count', 'sum']) -> pd.DataFrame:
    """
    分组统计分析
    
    Args:
        df: 输入的DataFrame
        group_by: 分组列名
        agg_columns: 要聚合的列，如果为None则使用所有数值列
        agg_funcs: 聚合函数列表
        
    Returns:
        分组统计结果
    """
    if group_by not in df.columns:
        raise ValueError(f"Group by column '{group_by}' not found in DataFrame")
    
    if agg_columns is None:
        agg_columns = df.select_dtypes(include=[np.number]).columns.tolist()
        # 排除分组列（如果是数值类型）
        if group_by in agg_columns:
            agg_columns.remove(group_by)
    
    if not agg_columns:
        return pd.DataFrame()
    
    # 过滤存在的列
    valid_agg_columns = [col for col in agg_columns if col in df.columns]
    
    if not valid_agg_columns:
        return pd.DataFrame()
    
    return df.groupby(group_by)[valid_agg_columns].agg(agg_funcs)


def hypothesis_test(df: pd.DataFrame, 
                   test_type: str,
                   **kwargs) -> Dict[str, Any]:
    """
    假设检验
    
    Args:
        df: 输入的DataFrame
        test_type: 检验类型 ('ttest_ind', 'ttest_rel', 'mannwhitneyu', 'chi2', 'anova')
        **kwargs: 检验相关的参数
        
    Returns:
        包含检验结果的字典
    """
    result = {'test_type': test_type, 'statistic': None, 'p_value': None, 'conclusion': ''}
    
    if test_type == 'ttest_ind':
        # 独立样本t检验
        group1_col = kwargs.get('group1_col')
        group2_col = kwargs.get('group2_col')
        group_col = kwargs.get('group_col')
        value_col = kwargs.get('value_col')
        
        if group1_col and group2_col:
            # 直接提供两组数据
            group1 = df[group1_col].dropna()
            group2 = df[group2_col].dropna()
        elif group_col and value_col:
            # 通过分组列和值列指定
            groups = df[group_col].unique()
            if len(groups) != 2:
                raise ValueError("Independent t-test requires exactly 2 groups")
            group1 = df[df[group_col] == groups[0]][value_col].dropna()
            group2 = df[df[group_col] == groups[1]][value_col].dropna()
        else:
            raise ValueError("Either (group1_col, group2_col) or (group_col, value_col) must be provided")
        
        statistic, p_value = stats.ttest_ind(group1, group2, equal_var=False)
        result.update({
            'statistic': statistic,
            'p_value': p_value,
            'conclusion': f"{'Reject' if p_value < 0.05 else 'Fail to reject'} null hypothesis (p={p_value:.4f})"
        })
        
    elif test_type == 'ttest_rel':
        # 配对样本t检验
        group1_col = kwargs.get('group1_col')
        group2_col = kwargs.get('group2_col')
        
        if not group1_col or not group2_col:
            raise ValueError("Paired t-test requires group1_col and group2_col")
            
        group1 = df[group1_col].dropna()
        group2 = df[group2_col].dropna()
        
        # 确保长度相同
        min_len = min(len(group1), len(group2))
        group1 = group1[:min_len]
        group2 = group2[:min_len]
        
        statistic, p_value = stats.ttest_rel(group1, group2)
        result.update({
            'statistic': statistic,
            'p_value': p_value,
            'conclusion': f"{'Reject' if p_value < 0.05 else 'Fail to reject'} null hypothesis (p={p_value:.4f})"
        })
        
    elif test_type == 'mannwhitneyu':
        # Mann-Whitney U检验
        group1_col = kwargs.get('group1_col')
        group2_col = kwargs.get('group2_col')
        group_col = kwargs.get('group_col')
        value_col = kwargs.get('value_col')
        
        if group1_col and group2_col:
            group1 = df[group1_col].dropna()
            group2 = df[group2_col].dropna()
        elif group_col and value_col:
            groups = df[group_col].unique()
            if len(groups) != 2:
                raise ValueError("Mann-Whitney U test requires exactly 2 groups")
            group1 = df[df[group_col] == groups[0]][value_col].dropna()
            group2 = df[df[group_col] == groups[1]][value_col].dropna()
        else:
            raise ValueError("Either (group1_col, group2_col) or (group_col, value_col) must be provided")
        
        statistic, p_value = stats.mannwhitneyu(group1, group2, alternative='two-sided')
        result.update({
            'statistic': statistic,
            'p_value': p_value,
            'conclusion': f"{'Reject' if p_value < 0.05 else 'Fail to reject'} null hypothesis (p={p_value:.4f})"
        })
        
    elif test_type == 'chi2':
        # 卡方检验
        observed_col = kwargs.get('observed_col')
        expected_col = kwargs.get('expected_col')
        contingency_table = kwargs.get('contingency_table')
        
        if contingency_table is not None:
            chi2, p_value, dof, expected = stats.chi2_contingency(contingency_table)
        elif observed_col and expected_col:
            observed = df[observed_col].values
            expected = df[expected_col].values
            chi2, p_value = stats.chisquare(observed, expected)
            dof = len(observed) - 1
        else:
            raise ValueError("Chi-square test requires either contingency_table or (observed_col, expected_col)")
        
        result.update({
            'statistic': chi2,
            'p_value': p_value,
            'degrees_of_freedom': dof,
            'conclusion': f"{'Reject' if p_value < 0.05 else 'Fail to reject'} null hypothesis (p={p_value:.4f})"
        })
        
    elif test_type == 'anova':
        # 单因素方差分析
        group_col = kwargs.get('group_col')
        value_col = kwargs.get('value_col')
        
        if not group_col or not value_col:
            raise ValueError("ANOVA requires group_col and value_col")
            
        groups = df[group_col].unique()
        if len(groups) < 2:
            raise ValueError("ANOVA requires at least 2 groups")
            
        group_data = [df[df[group_col] == group][value_col].dropna() for group in groups]
        statistic, p_value = stats.f_oneway(*group_data)
        
        result.update({
            'statistic': statistic,
            'p_value': p_value,
            'num_groups': len(groups),
            'conclusion': f"{'Reject' if p_value < 0.05 else 'Fail to reject'} null hypothesis (p={p_value:.4f})"
        })
    
    return result


def distribution_analysis(df: pd.DataFrame, 
                         column: str,
                         test_normality: bool = True) -> Dict[str, Any]:
    """
    数据分布分析
    
    Args:
        df: 输入的DataFrame
        column: 要分析的列名
        test_normality: 是否进行正态性检验
        
    Returns:
        包含分布分析结果的字典
    """
    if column not in df.columns:
        raise ValueError(f"Column '{column}' not found in DataFrame")
    
    series = df[column].dropna()
    
    result = {
        'column': column,
        'sample_size': len(series),
        'mean': series.mean(),
        'median': series.median(),
        'std': series.std(),
        'skewness': series.skew(),
        'kurtosis': series.kurtosis()
    }
    
    if test_normality and len(series) >= 8:
        # Shapiro-Wilk正态性检验（适用于小样本）
        if len(series) <= 5000:
            stat, p_value = stats.shapiro(series)
            result['normality_test'] = {
                'test': 'Shapiro-Wilk',
                'statistic': stat,
                'p_value': p_value,
                'is_normal': p_value > 0.05
            }
        else:
            # 对于大样本使用Kolmogorov-Smirnov检验
            stat, p_value = stats.kstest(series, 'norm', args=(series.mean(), series.std()))
            result['normality_test'] = {
                'test': 'Kolmogorov-Smirnov',
                'statistic': stat,
                'p_value': p_value,
                'is_normal': p_value > 0.05
            }
    
    return result


def comprehensive_analysis(df: pd.DataFrame, 
                         columns: Optional[List[str]] = None,
                         group_by: Optional[str] = None) -> Dict[str, Any]:
    """
    综合统计分析
    
    Args:
        df: 输入的DataFrame
        columns: 要分析的列，如果为None则分析所有数值列
        group_by: 分组列名（可选）
        
    Returns:
        包含综合分析结果的字典
    """
    result = {}
    
    # 描述性统计
    desc_stats = descriptive_statistics(df, columns)
    result['descriptive_statistics'] = desc_stats
    
    # 相关性分析
    corr_matrix = correlation_analysis(df, columns)
    result['correlation_matrix'] = corr_matrix
    
    # 分组统计（如果指定了分组列）
    if group_by and group_by in df.columns:
        group_stats = group_statistics(df, group_by, columns)
        result['group_statistics'] = group_stats
    
    # 分布分析（对每个数值列）
    if columns is None:
        numeric_cols = df.select_dtypes(include=[np.number]).columns.tolist()
    else:
        numeric_cols = [col for col in columns if col in df.select_dtypes(include=[np.number]).columns]
    
    distribution_results = {}
    for col in numeric_cols:
        try:
            dist_result = distribution_analysis(df, col)
            distribution_results[col] = dist_result
        except Exception as e:
            distribution_results[col] = {'error': str(e)}
    
    result['distribution_analysis'] = distribution_results
    
    return result


if __name__ == "__main__":
    # 示例用法
    np.random.seed(42)
    sample_data = {
        'group': ['A', 'A', 'B', 'B', 'A', 'B', 'A', 'B'],
        'value1': [10, 12, 15, 18, 11, 16, 13, 17],
        'value2': [20, 22, 25, 28, 21, 26, 23, 27],
        'category': ['X', 'Y', 'X', 'Y', 'X', 'Y', 'X', 'Y']
    }
    
    df = pd.DataFrame(sample_data)
    print("原始数据:")
    print(df)
    
    print("\n描述性统计:")
    desc_stats = descriptive_statistics(df)
    print(desc_stats)
    
    print("\n相关性矩阵:")
    corr = correlation_analysis(df)
    print(corr)
    
    print("\n分组统计:")
    group_stats = group_statistics(df, 'group')
    print(group_stats)
    
    print("\n分布分析 (value1):")
    dist_analysis = distribution_analysis(df, 'value1')
    print(dist_analysis)