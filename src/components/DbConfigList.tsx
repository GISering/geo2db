import { useState, useEffect } from 'react';
import { Card, Button, List, Input, Radio, Form, Space, Typography, Tag } from 'antd';
import { DeleteOutlined, PlusOutlined, CheckCircleOutlined, WarningOutlined } from '@ant-design/icons';
import type { DbConfig, DbType, NamedDbConfig, ConnectionTestResult, DamengDriverStatus } from '../types';

const { Text } = Typography;

interface DbConfigListProps {
  configs: NamedDbConfig[];
  activeConfig: string;
  onSelect: (name: string) => void;
  onDelete: (name: string) => Promise<boolean>;
  onAddNew: () => void;
  onBack: () => void;
}

export function DbConfigList({
  configs,
  activeConfig,
  onSelect,
  onDelete,
  onAddNew,
  onBack,
}: DbConfigListProps) {
  const [deleting, setDeleting] = useState<string | null>(null);

  const handleDelete = async (name: string) => {
    if (name === activeConfig && configs.length <= 1) {
      alert('无法删除最后一个配置');
      return;
    }
    setDeleting(name);
    const success = await onDelete(name);
    setDeleting(null);
    if (!success) {
      alert('删除失败');
    }
  };

  return (
    <Card
      title="数据库配置管理"
      extra={
        <Space>
          <Button type="primary" icon={<PlusOutlined />} onClick={onAddNew}>
            添加新配置
          </Button>
          <Button onClick={onBack}>返回</Button>
        </Space>
      }
    >
      <List
        dataSource={configs}
        locale={{ emptyText: '暂无保存的配置' }}
        renderItem={(config) => (
          <List.Item
            style={{
              cursor: 'pointer',
              background: config.name === activeConfig ? '#f0f5ff' : undefined,
              borderRadius: 8,
              padding: '12px 16px',
              marginBottom: 8,
            }}
            onClick={() => onSelect(config.name)}
            actions={[
              config.name === activeConfig ? (
                <Text type="success" key="active">当前使用</Text>
              ) : null,
              <Button
                key="delete"
                type="text"
                danger
                icon={<DeleteOutlined />}
                onClick={(e) => {
                  e.stopPropagation();
                  handleDelete(config.name);
                }}
                loading={deleting === config.name}
              >
                {deleting === config.name ? '删除中...' : '删除'}
              </Button>,
            ].filter(Boolean)}
          >
            <List.Item.Meta
              title={config.name}
              description={`${config.host}:${config.port}/${config.database}`}
            />
          </List.Item>
        )}
      />
    </Card>
  );
}

interface DbConfigEditorProps {
  config: DbConfig;
  configName: string;
  onChange: (config: DbConfig) => void;
  onNameChange: (name: string) => void;
  onTestConnection: () => Promise<any>;
  onSave: () => Promise<boolean>;
  onCancel: () => void;
  isTesting: boolean;
  testResult: ConnectionTestResult | null;
  isNew?: boolean;
  damengDriverStatus: DamengDriverStatus | null;
  onCheckDamengDriver: () => Promise<DamengDriverStatus | null>;
}

export function DbConfigEditor({
  config,
  configName,
  onChange,
  onNameChange,
  onTestConnection,
  onSave,
  onCancel,
  isTesting,
  testResult: _testResult,
  isNew = false,
  damengDriverStatus,
  onCheckDamengDriver,
}: DbConfigEditorProps) {
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

  // 当选择达梦数据库时，检测驱动
  useEffect(() => {
    if (config.db_type === 'Dameng') {
      onCheckDamengDriver();
    }
  }, [config.db_type, onCheckDamengDriver]);

  return (
    <Card
      title={isNew ? '添加新配置' : '编辑配置'}
      extra={<Button onClick={onCancel}>返回</Button>}
    >
      <Form layout="vertical">
        <Form.Item label="配置名称" required>
          <Input
            value={configName}
            onChange={(e) => onNameChange(e.target.value)}
            placeholder="输入配置名称"
          />
        </Form.Item>

        <Form.Item label="数据库类型">
          <Radio.Group
            value={config.db_type}
            onChange={(e) => handleChange('db_type', e.target.value)}
          >
            <Radio value="PostgreSQL">PostgreSQL / PostGIS</Radio>
            <Radio value="Dameng">
              达梦数据库
              {config.db_type === 'Dameng' && damengDriverStatus && (
                <Tag
                  color={damengDriverStatus.installed ? 'success' : 'warning'}
                  style={{ marginLeft: '8px' }}
                  icon={damengDriverStatus.installed ? <CheckCircleOutlined /> : <WarningOutlined />}
                >
                  {damengDriverStatus.message}
                </Tag>
              )}
            </Radio>
          </Radio.Group>
          {config.db_type === 'Dameng' && !damengDriverStatus?.installed && (
            <div style={{
              marginTop: '12px',
              padding: '12px',
              backgroundColor: '#fffbe6',
              border: '1px solid #ffe58f',
              borderRadius: '4px',
              fontSize: '13px',
              color: '#595959'
            }}>
              <div style={{ marginBottom: '8px', fontWeight: 'bold', color: '#d48806' }}>
                达梦数据库 ODBC 驱动安装说明：
              </div>
              <div style={{ marginBottom: '8px' }}>
                <strong>1. 下载驱动</strong>
                <div style={{ marginLeft: '16px', marginTop: '4px' }}>
                  <a
                    href="https://dn.navicat.com/drivers/dameng_odbc_win.zip"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    https://dn.navicat.com/drivers/dameng_odbc_win.zip
                  </a>
                </div>
              </div>
              <div style={{ marginBottom: '8px' }}>
                <strong>2. 解压缩包</strong>
                <div style={{ marginLeft: '16px', marginTop: '4px' }}>
                  将下载的 .zip 文件内容解压到计算机上的某个位置，例如：C:\dameng_odbc
                </div>
              </div>
              <div>
                <strong>3. 运行安装脚本</strong>
                <div style={{ marginLeft: '16px', marginTop: '4px' }}>
                  定位到解压的驱动文件所在路径，双击 <code style={{ backgroundColor: '#f5f5f5', padding: '2px 6px', borderRadius: '3px' }}>install_odbc.bat</code> 文件开始安装。
                </div>
              </div>
            </div>
          )}
        </Form.Item>

        <Space size="middle" style={{ width: '100%' }}>
          <Form.Item label="主机地址" style={{ flex: 1 }}>
            <Input
              value={config.host}
              onChange={(e) => handleChange('host', e.target.value)}
              placeholder="localhost"
            />
          </Form.Item>
          <Form.Item label="端口" style={{ width: 100 }}>
            <Input
              type="number"
              value={config.port}
              onChange={(e) => handleChange('port', parseInt(e.target.value) || 5432)}
              placeholder="5432"
            />
          </Form.Item>
        </Space>

        <Form.Item label="数据库名称">
          <Input
            value={config.database}
            onChange={(e) => handleChange('database', e.target.value)}
            placeholder="gis"
          />
        </Form.Item>

        <Form.Item label="用户名">
          <Input
            value={config.username}
            onChange={(e) => handleChange('username', e.target.value)}
            placeholder="postgres"
          />
        </Form.Item>

        <Form.Item label="密码">
          <Input.Password
            value={config.password}
            onChange={(e) => handleChange('password', e.target.value)}
            placeholder="输入密码"
          />
        </Form.Item>

        <Space>
          <Button
            onClick={onTestConnection}
            disabled={isTesting || !configName}
            loading={isTesting}
          >
            测试连接
          </Button>
          <Button
            type="primary"
            onClick={onSave}
            disabled={!configName}
          >
            保存
          </Button>
        </Space>
      </Form>
    </Card>
  );
}