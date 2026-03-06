import { useState } from 'react';
import { Button } from 'antd';
import { invoke } from '@tauri-apps/api/core';
import { useImport } from './hooks/useImport';
import { Header, StepBar } from './components/Header';
import { FileSelector } from './components/FileSelector';
import { DbConfigEditor } from './components/DbConfigList';
import { ImportConfigPanel } from './components/ImportConfig';
import { ProgressPanel } from './components/Progress';
import './App.css';

function App() {
  const {
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
    testConnection,
    testConnectionForConfig,
    isTestingConnection,
    connectionResult,
    tableName,
    setTableName,
    srs,
    setSrs,
    importMode,
    setImportMode,
    isImporting,
    result,
    startImport,
    reset,
    importProgress,
    damengDriverStatus,
    checkDamengDriver,
  } = useImport();

  // 状态：是否显示添加/编辑数据源弹窗
  const [showDbConfigEditor, setShowDbConfigEditor] = useState(false);

  const canProceedToImport = files.length > 0;

  // 取消导入
  const handleCancelImport = async () => {
    try {
      await invoke('cancel_import');
      reset();
    } catch (e) {
      console.error('取消导入失败:', e);
    }
  };

  const handleNextStep = () => {
    if (step === 'files' && canProceedToImport) {
      setStep('import');
    }
  };

  // 添加新数据源
  const handleAddDataSource = () => {
    // 清空当前配置为默认值
    setDbConfig({
      db_type: 'PostgreSQL',
      host: 'localhost',
      port: 5432,
      database: 'gis',
      username: 'postgres',
      password: '',
    });
    setConfigName('');
    setShowDbConfigEditor(true);
  };

  // 选择数据源
  const handleSelectDataSource = (name: string) => {
    selectConfig(name);
  };

  // 保存数据源配置
  const handleSaveDataSource = async (): Promise<boolean> => {
    const name = configName.trim() || '未命名配置';
    const success = await saveCurrentConfig(name);
    if (success) {
      setShowDbConfigEditor(false);
      return true;
    } else {
      alert('保存失败');
      return false;
    }
  };

  // 取消编辑数据源
  const handleCancelEditDataSource = () => {
    setShowDbConfigEditor(false);
  };

  // 渲染添加/编辑数据源弹窗
  if (showDbConfigEditor) {
    return (
      <div className="app">
        <Header />
        <main className="main-content">
          <div className="step-content">
            <DbConfigEditor
              config={dbConfig}
              configName={configName}
              onChange={setDbConfig}
              onNameChange={setConfigName}
              onTestConnection={testConnection as () => Promise<boolean>}
              onSave={handleSaveDataSource}
              onCancel={handleCancelEditDataSource}
              isTesting={isTestingConnection}
              testResult={connectionResult}
              isNew={true}
              damengDriverStatus={damengDriverStatus}
              onCheckDamengDriver={checkDamengDriver}
            />
          </div>
        </main>
        <footer className="app-footer">
          <span>空间数据导入工具 v0.1.0</span>
        </footer>
      </div>
    );
  }

  return (
    <div className="app">
      <Header />

      <main className="main-content">
        <StepBar currentStep={step} />
        {step === 'files' && (
          <div className="step-content">
            <FileSelector
              files={files}
              selectedFile={selectedFile}
              onSelectFile={setSelectedFile}
              onFilesSelected={selectFiles}
              layers={layers}
              selectedLayer={selectedLayer}
              onSelectLayer={selectLayer}
            />
            {canProceedToImport && (
              <div className="step-actions">
                <Button type="primary" onClick={handleNextStep}>
                  下一步: 导入配置
                </Button>
              </div>
            )}
          </div>
        )}

        {step === 'import' && (
          <div className="step-content">
            <ImportConfigPanel
              onPrevStep={() => setStep('files')}
              file={selectedFile}
              tableName={tableName}
              onTableNameChange={setTableName}
              srs={srs}
              onSrsChange={setSrs}
              importMode={importMode}
              onImportModeChange={setImportMode}
              onStartImport={startImport}
              isImporting={isImporting}
              dbConfig={dbConfig}
              configList={configList}
              configName={configName}
              onSelectDataSource={handleSelectDataSource}
              onAddDataSource={handleAddDataSource}
              connectionResult={connectionResult}
              onTestConnection={testConnection}
              onTestConnectionForConfig={testConnectionForConfig}
              onDeleteDataSource={deleteConfig}
              isTestingConnection={isTestingConnection}
            />
          </div>
        )}

        {step === 'progress' && (
          <div className="step-content">
            <ProgressPanel
              isImporting={isImporting}
              progress={importProgress}
              result={result}
              onReset={reset}
              onCancel={handleCancelImport}
            />
          </div>
        )}
      </main>

      <footer className="app-footer">
        <span>空间数据导入工具 v0.1.0</span>
      </footer>
    </div>
  );
}

export default App;