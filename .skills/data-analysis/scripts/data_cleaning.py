#!/usr/bin/env python3
"""
数据清洗功能模块

提供常用的数据清洗功能，包括：
- 处理缺失值
- 去除重复值
- 数据类型转换
- 异常值处理
- 字符串清理
"""

import pandas as pd
import numpy as np
from typing import List, Dict, Any, Optional, Union


def handle_missing_values(df: pd.DataFrame, strategy: str = 'drop', 
                         columns: Optional[List[str]] = None) -> pd.DataFrame:
    """
    处理缺失值
    
    Args:
        df: 输入的DataFrame
        strategy: 处理策略 ('drop', 'fill_mean', 'fill_median', 'fill_mode', 'fill_zero')
        columns: 指定要处理的列，如果为None则处理所有列
        
    Returns:
        处理后的DataFrame
    """
    df_clean = df.copy()
    
    if columns is None:
        columns = df_clean.columns.tolist()
    
    if strategy == 'drop':
        # 删除包含缺失值的行
        df_clean = df_clean.dropna(subset=columns)
    elif strategy == 'fill_mean':
        # 用均值填充数值列
        numeric_cols = df_clean[columns].select_dtypes(include=[np.number]).columns
        for col in numeric_cols:
            df_clean[col].fillna(df_clean[col].mean(), inplace=True)
    elif strategy == 'fill_median':
        # 用中位数填充数值列
        numeric_cols = df_clean[columns].select_dtypes(include=[np.number]).columns
        for col in numeric_cols:
            df_clean[col].fillna(df_clean[col].median(), inplace=True)
    elif strategy == 'fill_mode':
        # 用众数填充
        for col in columns:
            mode_val = df_clean[col].mode()
            if len(mode_val) > 0:
                df_clean[col].fillna(mode_val[0], inplace=True)
    elif strategy == 'fill_zero':
        # 用0填充
        for col in columns:
            df_clean[col].fillna(0, inplace=True)
    
    return df_clean


def remove_duplicates(df: pd.DataFrame, subset: Optional[List[str]] = None, 
                     keep: str = 'first') -> pd.DataFrame:
    """
    去除重复值
    
    Args:
        df: 输入的DataFrame
        subset: 指定用于判断重复的列，如果为None则考虑所有列
        keep: 保留策略 ('first', 'last', False)
        
    Returns:
        去重后的DataFrame
    """
    return df.drop_duplicates(subset=subset, keep=keep).reset_index(drop=True)


def convert_data_types(df: pd.DataFrame, type_mapping: Dict[str, str]) -> pd.DataFrame:
    """
    转换数据类型
    
    Args:
        df: 输入的DataFrame
        type_mapping: 列名到数据类型的映射字典
        
    Returns:
        转换类型后的DataFrame
    """
    df_converted = df.copy()
    for column, dtype in type_mapping.items():
        if column in df_converted.columns:
            try:
                if dtype == 'datetime':
                    df_converted[column] = pd.to_datetime(df_converted[column])
                else:
                    df_converted[column] = df_converted[column].astype(dtype)
            except Exception as e:
                print(f"Warning: Failed to convert column '{column}' to {dtype}: {e}")
    
    return df_converted


def handle_outliers(df: pd.DataFrame, columns: Optional[List[str]] = None, 
                   method: str = 'iqr', threshold: float = 1.5) -> pd.DataFrame:
    """
    处理异常值
    
    Args:
        df: 输入的DataFrame
        columns: 指定要处理的数值列，如果为None则自动检测数值列
        method: 异常值检测方法 ('iqr', 'zscore')
        threshold: 阈值 (IQR方法使用1.5倍IQR，Z-score方法通常使用3)
        
    Returns:
        处理异常值后的DataFrame（将异常值替换为NaN）
    """
    df_outlier = df.copy()
    
    if columns is None:
        columns = df_outlier.select_dtypes(include=[np.number]).columns.tolist()
    
    for col in columns:
        if col not in df_outlier.columns:
            continue
            
        if method == 'iqr':
            Q1 = df_outlier[col].quantile(0.25)
            Q3 = df_outlier[col].quantile(0.75)
            IQR = Q3 - Q1
            lower_bound = Q1 - threshold * IQR
            upper_bound = Q3 + threshold * IQR
            
            # 将异常值替换为NaN
            df_outlier[col] = df_outlier[col].apply(
                lambda x: np.nan if x < lower_bound or x > upper_bound else x
            )
            
        elif method == 'zscore':
            mean_val = df_outlier[col].mean()
            std_val = df_outlier[col].std()
            if std_val == 0:
                continue
                
            z_scores = (df_outlier[col] - mean_val) / std_val
            df_outlier[col] = df_outlier[col].where(abs(z_scores) <= threshold, np.nan)
    
    return df_outlier


def clean_strings(df: pd.DataFrame, columns: Optional[List[str]] = None,
                 operations: List[str] = ['strip', 'lower']) -> pd.DataFrame:
    """
    清理字符串数据
    
    Args:
        df: 输入的DataFrame
        columns: 指定要清理的字符串列，如果为None则自动检测字符串列
        operations: 要执行的操作列表 ('strip', 'lower', 'upper', 'remove_extra_spaces')
        
    Returns:
        清理后的DataFrame
    """
    df_clean = df.copy()
    
    if columns is None:
        columns = df_clean.select_dtypes(include=['object']).columns.tolist()
    
    for col in columns:
        if col not in df_clean.columns:
            continue
            
        series = df_clean[col].astype(str)
        
        for op in operations:
            if op == 'strip':
                series = series.str.strip()
            elif op == 'lower':
                series = series.str.lower()
            elif op == 'upper':
                series = series.str.upper()
            elif op == 'remove_extra_spaces':
                series = series.str.replace(r'\s+', ' ', regex=True)
        
        # 将原始的NaN值恢复
        series = series.where(df_clean[col].notna(), df_clean[col])
        df_clean[col] = series
    
    return df_clean


def comprehensive_cleaning(df: pd.DataFrame, 
                          missing_strategy: str = 'drop',
                          handle_duplicates: bool = True,
                          outlier_method: Optional[str] = None,
                          string_operations: Optional[List[str]] = None) -> pd.DataFrame:
    """
    综合数据清洗函数
    
    Args:
        df: 输入的DataFrame
        missing_strategy: 缺失值处理策略
        handle_duplicates: 是否处理重复值
        outlier_method: 异常值处理方法，None表示不处理
        string_operations: 字符串清理操作
        
    Returns:
        清洗后的DataFrame
    """
    df_clean = df.copy()
    
    # 处理缺失值
    df_clean = handle_missing_values(df_clean, strategy=missing_strategy)
    
    # 处理重复值
    if handle_duplicates:
        df_clean = remove_duplicates(df_clean)
    
    # 处理异常值
    if outlier_method:
        df_clean = handle_outliers(df_clean, method=outlier_method)
    
    # 清理字符串
    if string_operations:
        df_clean = clean_strings(df_clean, operations=string_operations)
    
    return df_clean


if __name__ == "__main__":
    # 示例用法
    sample_data = {
        'name': ['Alice', 'Bob', 'Charlie', 'Alice', None],
        'age': [25, 30, np.nan, 25, 35],
        'salary': [50000, 60000, 55000, 50000, 70000],
        'city': [' New York ', 'LOS ANGELES', 'Chicago', ' New York ', 'Miami']
    }
    
    df = pd.DataFrame(sample_data)
    print("原始数据:")
    print(df)
    print("\n清洗后的数据:")
    cleaned_df = comprehensive_cleaning(
        df, 
        missing_strategy='fill_mean',
        handle_duplicates=True,
        outlier_method='iqr',
        string_operations=['strip', 'lower', 'remove_extra_spaces']
    )
    print(cleaned_df)