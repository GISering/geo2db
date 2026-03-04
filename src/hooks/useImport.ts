import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { message } from 'antd';
import type {
  FileInfo,
  DbConfig,
  DbConfigList,
  NamedDbConfig,
  ImportConfig,
  ImportResult,
  ConnectionTestResult,
  AppStep,
  ImportMode,
  LayerInfo,
  ImportProgress,
} from '../types';

export function useImport() {
  const [step, setStep] = useState<AppStep>('files');
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [selectedFile, setSelectedFile] = useState<FileInfo | null>(null);
  const [layers, setLayers] = useState<LayerInfo[]>([]);
  const [selectedLayer, setSelectedLayer] = useState<string | null>(null);
  const [dbConfig, setDbConfig] = useState<DbConfig>({
    db_type: 'PostgreSQL',
    host: 'localhost',
    port: 10001,
    database: 'gis',
    username: 'postgres',
    password: '',
  });
  const [configName, setConfigName] = useState<string>('默认配置');
  const [configList, setConfigList] = useState<NamedDbConfig[]>([]);
  const [tableName, setTableName] = useState('');
  const [srs, setSrs] = useState('EPSG:4326');
  const [importMode, setImportMode] = useState<ImportMode>('CreateNew');
  const [isImporting, setIsImporting] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [isTestingConnection, setIsTestingConnection] = useState(false);
  const [connectionResult, setConnectionResult] = useState<ConnectionTestResult | null>(null);
  const [importProgress, setImportProgress] = useState<ImportProgress | null>(null);

  // 监听导入进度事件
  useEffect(() => {
    const unlisten = listen<ImportProgress>('import-progress', (event) => {
      setImportProgress(event.payload);
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  // 监听导入完成事件
  useEffect(() => {
    const unlisten = listen<ImportResult>('import-complete', (event) => {
      setResult(event.payload);
      setIsImporting(false);
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  // 初始化时加载保存的配置列表
  useEffect(() => {
    const loadSavedConfigs = async () => {
      try {
        const configs = await invoke<DbConfigList>('load_config');
        setConfigList(configs.configs);

        // 如果有活动配置，加载它
        if (configs.active_config) {
          const activeConfig = configs.configs.find(c => c.name === configs.active_config);
          if (activeConfig) {
            // NamedDbConfig extends DbConfig, so we can use it directly
            const { name, ...dbConfigWithoutName } = activeConfig;
            setDbConfig(dbConfigWithoutName);
            setConfigName(activeConfig.name);
          }
        }
      } catch (e) {
        console.warn('加载保存的配置失败:', e);
      }
    };

    loadSavedConfigs();
  }, []);

  // 当选择文件变化时，获取图层列表
  useEffect(() => {
    if (!selectedFile) {
      setLayers([]);
      setSelectedLayer(null);
      return;
    }

    const fetchLayers = async () => {
      try {
        const fileLayers = await invoke<LayerInfo[]>('list_layers', { path: selectedFile.path });
        setLayers(fileLayers);
        if (fileLayers.length > 0) {
          setSelectedLayer(fileLayers[0].name);
        }
      } catch (e) {
        console.warn('获取图层列表失败:', e);
        setLayers([]);
      }
    };

    fetchLayers();
  }, [selectedFile?.path]);

  // 选择文件
  const selectFiles = useCallback(async (paths: string[]) => {
    try {
      const fileInfos = await invoke<FileInfo[]>('list_files', { paths });
      setFiles(fileInfos);

      if (fileInfos.length > 0) {
        setSelectedFile(fileInfos[0]);
        setTableName(fileInfos[0].name.toLowerCase().replace(/[^a-z0-9]/g, '_'));
      }
    } catch (error) {
      console.error('选择文件失败:', error);
    }
  }, []);

  // 选择图层
  const selectLayer = useCallback(async (layerName: string) => {
    if (!selectedFile) return;

    setSelectedLayer(layerName);
    try {
      const fileInfo = await invoke<FileInfo>('get_file_info', {
        path: selectedFile.path,
        layerName
      });
      setSelectedFile(fileInfo);
      setTableName(fileInfo.name.toLowerCase().replace(/[^a-z0-9]/g, '_'));
    } catch (error) {
      console.error('切换图层失败:', error);
    }
  }, [selectedFile]);

  // 测试数据库连接
  const testConnection = useCallback(async (): Promise<boolean> => {
    setIsTestingConnection(true);
    setConnectionResult(null);
    try {
      const result = await invoke<ConnectionTestResult>('test_connection', { config: dbConfig });
      setConnectionResult(result);

      // 根据连接结果显示气泡提示
      if (result.success) {
        message.success('连接成功', 3);
      } else {
        message.error('连接失败: ' + result.message, 5);
      }

      // 连接成功后自动保存配置
      if (result.success) {
        try {
          await invoke('save_config', { config: dbConfig, name: configName });
          // 刷新配置列表
          const configs = await invoke<DbConfigList>('load_config');
          setConfigList(configs.configs);
        } catch (e) {
          console.warn('保存配置失败:', e);
        }
      }

      const success = result.success;
      return success;
    } catch (error) {
      setConnectionResult({
        success: false,
        message: String(error),
        server_version: null,
      });
      return false;
    } finally {
      setIsTestingConnection(false);
    }
  }, [dbConfig, configName]);

  // 保存配置
  const saveCurrentConfig = useCallback(async (name: string) => {
    console.log('saveCurrentConfig called with:', { name, dbConfig });
    try {
      await invoke('save_config', { config: dbConfig, name });
      console.log('save_config invoked successfully');
      const configs = await invoke<DbConfigList>('load_config');
      console.log('load_config result:', configs);
      setConfigList(configs.configs);
      setConfigName(name);
      return true;
    } catch (e) {
      console.error('保存配置失败:', e);
      return false;
    }
  }, [dbConfig]);

  // 删除配置
  const deleteConfig = useCallback(async (name: string) => {
    try {
      await invoke('delete_config', { name });
      const configs = await invoke<DbConfigList>('load_config');
      setConfigList(configs.configs);
      return true;
    } catch (e) {
      console.error('删除配置失败:', e);
      return false;
    }
  }, []);

  // 选择配置
  const selectConfig = useCallback((name: string) => {
    const config = configList.find(c => c.name === name);
    if (config) {
      // NamedDbConfig extends DbConfig, so we can use it directly
      const { name: _, ...dbConfigWithoutName } = config;
      setDbConfig(dbConfigWithoutName);
      setConfigName(name);
    }
  }, [configList]);

  // 执行导入
  const startImport = useCallback(async () => {
    if (!selectedFile) return;

    setIsImporting(true);
    setResult(null);
    setStep('progress');

    try {
      const config: ImportConfig = {
        db_config: dbConfig,
        file_path: selectedFile.path,
        layer_name: selectedFile.layer_name || null,
        table_name: tableName,
        srs: srs,
        import_mode: importMode,
        field_mapping: null,
      };

      // 启动异步导入，不等待结果，结果通过事件返回
      await invoke<ImportResult>('import_file', { config });
      // 注意：这里不设置 result，结果通过 import-complete 事件获取
    } catch (error) {
      setResult({
        success: false,
        imported_count: 0,
        error_count: 1,
        errors: [String(error)],
        duration_ms: 0,
      });
      setIsImporting(false);
    }
    // 注意：不使用 finally，让 isImporting 在事件回调中设置为 false
  }, [selectedFile, dbConfig, tableName, srs, importMode]);

  // 重置
  const reset = useCallback(() => {
    setStep('files');
    setFiles([]);
    setSelectedFile(null);
    setResult(null);
    setConnectionResult(null);
  }, []);

  return {
    step,
    setStep,
    files,
    selectedFile,
    setSelectedFile,
    selectFiles,
    layers,
    selectedLayer,
    selectLayer,
    dbConfig,
    setDbConfig,
    configName,
    setConfigName,
    configList,
    selectConfig,
    saveCurrentConfig,
    deleteConfig,
    tableName,
    setTableName,
    srs,
    setSrs,
    importMode,
    setImportMode,
    isImporting,
    result,
    startImport,
    testConnection,
    isTestingConnection,
    connectionResult,
    importProgress,
    reset,
  };
}