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
        newConfig.port = 5432;
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
            <span
              className="dameng-tip-icon"
              title="需要安装 ODBC 驱动，点击查看详情"
              style={{
                marginLeft: '6px',
                color: '#ad6800',
                fontSize: '12px',
                cursor: 'pointer'
              }}
              onClick={(e) => {
                e.preventDefault();
                handleChange('db_type', 'Dameng');
              }}
            >
              ⚠️ 需安装驱动
            </span>
          </label>
        </div>
        {config.db_type === 'Dameng' && (
          <div className="dameng-tip" style={{
            marginTop: '8px',
            padding: '12px',
            backgroundColor: '#fff7e6',
            border: '1px solid #ffd591',
            borderRadius: '4px',
            fontSize: '13px',
            color: '#595959'
          }}>
            <div style={{ marginBottom: '8px', fontWeight: 'bold', color: '#ad6800' }}>
              达梦数据库 ODBC 驱动安装说明：
            </div>
            <div style={{ marginBottom: '8px' }}>
              <strong>1. 下载驱动</strong>
              <div style={{ marginLeft: '16px', marginTop: '4px' }}>
                请从此链接下载 Windows 版本 ODBC 驱动程序包：
              </div>
              <div style={{ marginLeft: '16px', marginTop: '4px' }}>
                <a
                  href="https://dn.navicat.com/drivers/dameng_odbc_win.zip"
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ color: '#1890ff' }}
                >
                  https://dn.navicat.com/drivers/dameng_odbc_win.zip
                </a>
              </div>
            </div>
            <div>
              <strong>2. 解压缩包</strong>
              <div style={{ marginLeft: '16px', marginTop: '4px' }}>
                将下载的 .zip 文件内容解压到计算机上的某个位置，例如：C:\dameng_odbc
              </div>
            </div>
            <div style={{ marginTop: '8px' }}>
              <strong>3. 运行安装脚本</strong>
              <div style={{ marginLeft: '16px', marginTop: '4px' }}>
                定位到解压的驱动文件所在路径，双击 <code style={{ backgroundColor: '#f5f5f5', padding: '2px 6px', borderRadius: '3px' }}>install_odbc.bat</code> 文件开始安装。
              </div>
            </div>
          </div>
        )}
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
            onChange={(e) => handleChange('port', parseInt(e.target.value) || 5432)}
            placeholder={config.db_type === 'PostgreSQL' ? '5432' : '5236'}
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