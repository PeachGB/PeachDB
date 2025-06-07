import java.io.File;
import java.io.IOException;
import java.util.ArrayList;
import java.util.HashMap;

class Table{
    private static class Atributes{
        private enum datatype{
            INTEGER,
            FLOAT,
            TEXT,
            BOOLEAN,
            NULL,
        }
        boolean NOT_NULL;
        boolean unique;
        boolean autoIncrement;
        datatype datatype;

        Atributes(String[] atributes){

            for (String atribute : atributes){
                switch (atribute.toUpperCase()){
                    case "NOT NULL":
                        NOT_NULL = true;
                        break;
                    case "UNIQUE":
                        unique = true;
                        break;
                    case "AUTOINCREMENT":
                        autoIncrement = true;
                        break;
                    case "INTEGER": case "INT":
                        datatype = datatype.INTEGER;
                        break;
                    case "FLOAT":
                        datatype = datatype.FLOAT;
                        break;
                    case "TEXT":
                        datatype = datatype.TEXT;
                        break;
                    case "BOOLEAN":
                        datatype = datatype.BOOLEAN;
                        break;
                    case "NULL":
                        datatype = datatype.NULL;
                        break;
                }

            }

        }
        Atributes(){
            NOT_NULL = false;
            unique = false;
            autoIncrement = false;
            datatype = datatype.NULL;
        }
        public void notNull(boolean set){
            NOT_NULL = set;
        }
        public void unique(boolean set){
            unique = set;
        }
        public void autoIncrement(boolean set){
            autoIncrement = set;
        }
        public void datatype(String set){
            switch(set){
                case "int":
                    datatype = datatype.INTEGER;
                    break;
                case "float":
                    datatype = datatype.FLOAT;
                    break;
                case "text":
                    datatype = datatype.TEXT;
                    break;
                case "boolean":
                    datatype = datatype.BOOLEAN;
                    break;
                case "null":
                    datatype = datatype.NULL;
                    break;
            }
        }
    }
    String name;
    HashMap<String,Atributes> column_attributes;
    ArrayList<String> column_names;
    HashMap<String, ArrayList<String>> data;

    Table(String name){
        this.name = name;
        column_attributes = new HashMap<>();
        column_names = new ArrayList<>();
        data = new HashMap<>();
        addColumn("id", new String[]{"NOT NULL", "UNIQUE", "AUTOINCREMENT"});

    }

    void addColumn(String name){
        data.put(name, new ArrayList<>());
        column_names.add(name);
        column_attributes.put(name, new Atributes());
    }
    void addColumn(String name, String[] atributes){
        data.put(name, new ArrayList<>());
        column_names.add(name);
        column_attributes.put(name, new Atributes(atributes));
    }
    void modifyColumn(String name, String[] atributes){
        for (String atribute : atributes){
            switch (atribute.toUpperCase()){
                case "NOT NULL":
                    column_attributes.get(name).notNull(!column_attributes.get(name).NOT_NULL);
                    break;
                case "UNIQUE":
                    column_attributes.get(name).unique(!column_attributes.get(name).unique);
                    break;
                case "AUTOINCREMENT":
                    column_attributes.get(name).autoIncrement(!column_attributes.get(name).autoIncrement);
                    break;
                case "INTEGER": case "INT":
                    column_attributes.get(name).datatype("int");
                    break;
                case "FLOAT":
                    column_attributes.get(name).datatype("float");
                    break;
                case "TEXT":
                    column_attributes.get(name).datatype("text");
                    break;
                case "BOOLEAN":
                    column_attributes.get(name).datatype("boolean");
                    break;
                case "NULL":
                    column_attributes.get(name).datatype("null");
                    break;
                default:
                    throw new IllegalArgumentException("Invalid attribute");


            }
        }
    }
    void put(String[] where, String[] Data){
        data.get("id").add(String.valueOf(data.get("id").size()));
        for (int i = 0; i < where.length; i++) {
            if (column_attributes.get(where[i]).NOT_NULL && Data[i] == null){
                throw new IllegalArgumentException("Column "+where[i]+" is not nullable");
            }
            data.get(where[i]).add(Data[i]);

        }
    }


}
public final class Database {
    //PEACHDB VER 1.0 in hexadecimal
    private static Database instance;
    String header = "50 45 41 43 48 44 42 20 56 45 52 20 31 2E 30 00";
    byte header_size = 16;
    HashMap<String,Integer> table_Header = new HashMap<>();
    ArrayList<String> tables_names = new ArrayList<>();
    HashMap<String, Table> tables = new HashMap<>();

    int filesize;
    File database;


    private Database(){
    }


    public static Database getInstance(){
        if(instance == null){
            instance = new Database();
        }
        return instance;
    }


    public void createDatabase(String filename) throws IOException {

        database = new File(filename.concat(".db"));
        try{
            if(database.createNewFile()){
                System.out.println("Database file created successfully");
            }else{
                System.out.println("Database file already exists");
            }
        } catch (IOException e) {
            System.out.println("An error occurred while creating database file");
            e.printStackTrace();
        }

    }

    public void createTable(String table_name){
        tables.put(table_name, new Table(table_name));
        table_Header.put(table_name, tables.size());
        tables_names.add(table_name);

    }
    public void dropTable(String table_name){
        tables.remove(table_name);
        tables_names.remove(table_name);
    }
    public void addColumn(String table_name, String column_name){
        tables.get(table_name).addColumn(column_name);
    }
    public void addColumn(String table_name, String column_name, String[] atributes){
        tables.get(table_name).addColumn(column_name, atributes);
    }
    public void addData(String table_name, String[] where, String[] Data){
        tables.get(table_name).put(where, Data);
    }
    public void setColumnAtributes(String table_name, String column_name, String[] atributes){
        tables.get(table_name).modifyColumn(column_name, atributes);
    }






}
