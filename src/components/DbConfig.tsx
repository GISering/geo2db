import type { DbConfig, DbType, ConnectionTestResult, NamedDbConfig } from '../types';

interface DbConfigProps {
  config: DbConfig;
  onChange: (config: DbConfig) => void;
  onTestConnection: () => Promise<boolean>;
  onManageConfigs?: () => void;
  isTesting: boolean;
  testResult: ConnectionTestResult | null;
  configList?: NamedDbConfig[];
  onSelectConfig?: (name: string) => void;
  onAddConfig?: () => void;
}

export function DbConfigPanel({
  config,
  onChange,
  onTestConnection,
  onManageConfigs,
  isTesting,
  testResult,
  configList = [],
  onSelectConfig,
  onAddConfig,
}: DbConfigProps) {
  const handleChange = (field: keyof DbConfig, value: string | number | DbType) => {
    const newConfig: DbConfig = { ...config };
    // 如果选择数据库类型，默认端口自动设置
    if (field === 'db_type') {
      newConfig.db_type = value as DbType;
      if (value === 'PostgreSQL') {
        newConfig.port = 10001;
      } else if (value === 'Dameng') {
        newConfig.port = 5236;
      }
    } else {
      (newConfig as any)[field] = value;
    }
    onChange(newConfig);
  };

  const handleSelectChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    if (value === 'add_new' && onAddConfig) {
      onAddConfig();
    } else if (value && onSelectConfig) {
      onSelectConfig(value);
    }
  };

  return (
    <div className="db-config">
      <h2>数据库连接配置</h2>

      {/* 数据源选择区域 */}
      <div className="form-group">
        <label>数据源</label>
        <div className="datasource-select">
          <select
            value=""
            onChange={handleSelectChange}
            className="datasource-dropdown"
          >
            <option value="" disabled>选择已保存的数据源</option>
            {configList.map((item) => (
              <option key={item.name} value={item.name}>
                {item.name} ({item.host}:{item.port}/{item.database})
              </option>
            ))}
            <option value="add_new">+ 添加新数据源</option>
          </select>
          {onAddConfig && (
            <button
              className="btn-secondary btn-add-datasource"
              onClick={onAddConfig}
            >
              添加数据源
            </button>
          )}
        </div>
      </div>

      <div className="form-group">
        <label>数据库类型</label>
        <div className="radio-group">
          <label className="radio-label">
            <input
              type="radio"
              name="db_type"
              value="PostgreSQL"
              checked={config.db_type === 'PostgreSQL'}
              onChange={() => handleChange('db_type', 'PostgreSQL')}
            />
            <span>PostgreSQL / PostGIS</span>
          </label>
          <label className="radio-label">
            <input
              type="radio"
              name="db_type"
              value="Dameng"
              checked={config.db_type === 'Dameng'}
              onChange={() => handleChange('db_type', 'Dameng')}
            />
            <span>达梦数据库</span>
          </label>
        </div>
      </div>

      <div className="form-row">
        <div className="form-group">
          <label>主机地址</label>
          <input
            type="text"
            value={config.host}
            onChange={(e) => handleChange('host', e.target.value)}
            placeholder="localhost"
          />
        </div>
        <div className="form-group form-group-small">
          <label>端口</label>
          <input
            type="number"
            value={config.port}
            onChange={(e) => handleChange('port', parseInt(e.target.value) || 10001)}
            placeholder={config.db_type === 'PostgreSQL' ? '10001' : '5236'}
          />
        </div>
      </div>

      <div className="form-group">
        <label>数据库名称</label>
        <input
          type="text"
          value={config.database}
          onChange={(e) => handleChange('database', e.target.value)}
          placeholder="gis"
        />
      </div>

      <div className="form-group">
        <label>用户名</label>
        <input
          type="text"
          value={config.username}
          onChange={(e) => handleChange('username', e.target.value)}
          placeholder="postgres"
        />
      </div>

      <div className="form-group">
        <label>密码</label>
        <input
          type="password"
          value={config.password}
          onChange={(e) => handleChange('password', e.target.value)}
          placeholder="输入密码"
        />
      </div>

      <div className="form-actions">
        <button
          className="btn-secondary"
          onClick={onTestConnection}
          disabled={isTesting}
        >
          {isTesting ? '测试中...' : '测试连接'}
        </button>
        {onManageConfigs && (
          <button
            className="btn-secondary"
            onClick={onManageConfigs}
          >
            管理配置
          </button>
        )}
      </div>

      {testResult && (
        <div className={`test-result ${testResult.success ? 'success' : 'error'}`}>
          <span className="result-icon">{testResult.success ? '✓' : '✗'}</span>
          <span className="result-message">{testResult.message}</span>
          {testResult.server_version && (
            <div className="server-version">{testResult.server_version}</div>
          )}
        </div>
      )}
    </div>
  );
}