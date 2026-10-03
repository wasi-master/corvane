// Storage account with a blob service and soft delete.
targetScope = 'resourceGroup'
@description('Prefix used for every resource name.')
@minLength(3)
@maxLength(11)
param namePrefix string
@allowed([
  'Standard_LRS'
  'Standard_GRS'
])
param skuName string = 'Standard_LRS'
param location string = resourceGroup().location
param retentionDays int = 7
var storageName = toLower('${namePrefix}${uniqueString(resourceGroup().id)}')
var isProd = contains(namePrefix, 'prod') && retentionDays >= 30

/* The account itself; HTTPS only. */
resource storage 'Microsoft.Storage/storageAccounts@2023-01-01' = {
  name: storageName
  location: location
  sku: {
    name: skuName
  }
  kind: 'StorageV2'
  tags: {
    environment: isProd ? 'prod' : 'dev'
  }
  properties: {
    supportsHttpsTrafficOnly: true
    minimumTlsVersion: 'TLS1_2'
  }
}

resource blobService 'Microsoft.Storage/storageAccounts/blobServices@2023-01-01' = {
  parent: storage
  name: 'default'
  properties: {
    deleteRetentionPolicy: {
      enabled: retentionDays > 0
      days: retentionDays
    }
  }
}
output storageId string = storage.id
output containers array = [for i in range(0, 3): 'logs-${i}']
