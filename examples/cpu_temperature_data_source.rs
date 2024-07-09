use std::thread;

use anyhow::Context as _;
use open62541::{
    ua, DataSource, DataSourceError, DataSourceReadContext, DataSourceResult,
    DataSourceWriteContext, ObjectNode, Server, VariableNode,
};
use open62541_sys::{
    UA_NS0ID_BASEDATAVARIABLETYPE, UA_NS0ID_FOLDERTYPE, UA_NS0ID_OBJECTSFOLDER, UA_NS0ID_ORGANIZES,
    UA_NS0ID_STRING,
};

static mut CPU_TEMPERATURE: f32 = 39.0;
static mut INCREASING: bool = true;

#[derive(Default)]
struct ControllerDataSource {
    temperature: String,
}

impl DataSource for ControllerDataSource {
    fn read(&mut self, context: &mut DataSourceReadContext) -> DataSourceResult {
        println!("Reading cpu temperature value");

        self.temperature = read_cpu_temperature().to_string();

        let value = ua::Variant::scalar(
            // We do not expect strings with NUL bytes.
            ua::String::new(&self.temperature).map_err(|_| ua::StatusCode::BADINTERNALERROR)?,
        );
        context.set_variant(value);
        Ok(())
    }

    fn write(&mut self, _context: &mut DataSourceWriteContext) -> DataSourceResult {
        println!("Writing cpu temperature is not possible!");
        Err(DataSourceError::NotSupported)
    }
}

fn read_cpu_temperature() -> f32 {
    let cpu_temperature: f32;

    unsafe {
        println!("Current CPU Temperature: {CPU_TEMPERATURE:.2} °C");
        if CPU_TEMPERATURE >= 48.5 {
            INCREASING = false;
        } else if CPU_TEMPERATURE <= 38.5 {
            INCREASING = true;
        }
        if INCREASING {
            CPU_TEMPERATURE += 0.1;
        } else {
            CPU_TEMPERATURE -= 0.1;
        }
        cpu_temperature = CPU_TEMPERATURE;
    }
    cpu_temperature
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let (server, runner) = Server::new();

    println!("Adding server nodes");

    let object_node = ObjectNode {
        requested_new_node_id: ua::NodeId::string(1, "Controller"),
        parent_node_id: ua::NodeId::ns0(UA_NS0ID_OBJECTSFOLDER),
        reference_type_id: ua::NodeId::ns0(UA_NS0ID_ORGANIZES),
        browse_name: ua::QualifiedName::new(1, "Controller"),
        type_definition: ua::NodeId::ns0(UA_NS0ID_FOLDERTYPE),
        attributes: ua::ObjectAttributes::default(),
    };

    let cpu_temperature_variable = ua::NodeId::string(1, "cpu_temperature");
    let variable_node = VariableNode {
        requested_new_node_id: cpu_temperature_variable.clone(),
        parent_node_id: object_node.requested_new_node_id.clone(),
        reference_type_id: ua::NodeId::ns0(UA_NS0ID_ORGANIZES),
        browse_name: ua::QualifiedName::new(1, "temperature"),
        type_definition: ua::NodeId::ns0(UA_NS0ID_BASEDATAVARIABLETYPE),
        attributes: ua::VariableAttributes::default()
            .with_data_type(&ua::NodeId::ns0(UA_NS0ID_STRING))
            .with_access_level(
                &ua::AccessLevel::NONE
                    .with_current_read(true)
                    .with_current_write(true),
            ),
    };

    let data_source = ControllerDataSource::default();

    server
        .add_object_node(object_node)
        .context("add object node")?;
    server
        .add_data_source_variable_node(variable_node, data_source)
        .context("add variable node")?;

    // Start runner task that handles incoming connections (events).
    let runner_task_handle = thread::spawn(|| -> anyhow::Result<()> {
        println!("Running server");
        runner.run()?;
        Ok(())
    });

    // Wait for runner task to finish eventually (SIGINT/Ctrl+C).
    if let Err(err) = runner_task_handle
        .join()
        .expect("runner task should not panic")
    {
        println!("Runner task failed: {err}");
    }

    println!("Exiting");

    server
        .delete_node(&cpu_temperature_variable)
        .context("delete variable node")?;

    println!("Done");

    Ok(())
}
