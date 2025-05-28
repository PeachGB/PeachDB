public class Parser {

    int pos;
    int next_pos;
    int next_token_pos;
    String buffer;

    public Parser() {
        buffer = "";
        pos = 0;
        next_pos = pos + 1;
    }

    public void parse(String input) {
        buffer = input;
        lex();

    }
    private void advancePosition(int StringLength) {
        pos = pos + StringLength;
        next_pos = pos + 1;
    }
    private static Token parsetoken(String token){
        //TODO
        Token Todo = null;
        return Todo;
    }
    public void lex() {
        String[] words = buffer.split("\\s+");
        Token[] tokens = new Token[words.length];
        for (int i = 0; i < words.length; i++) {
        }

    }
}

abstract class Token<T> {
    T value;
    int position;

    Token(T val, int pos) {
        this.value = val;
        this.position = pos;

    }
}
class Keyword extends Token {

    public enum SQLKeyword {
        // Base Keywords
        SELECT,
        FROM,
        WHERE,
        INSERT,
        INTO,
        VALUES,
        UPDATE,
        SET,
        DELETE,

        // Joins
        JOIN,
        INNER,
        LEFT,
        RIGHT,
        FULL,
        OUTER,

        // Group and Order
        GROUP,
        BY,
        HAVING,
        ORDER,
        ASC,
        DESC,

        // Logical Operators
        AND,
        OR,
        NOT,
        IN,
        BETWEEN,
        LIKE,
        IS,
        NULL,

        // Aggregator Funcs
        COUNT,
        SUM,
        AVG,
        MAX,
        MIN,

        // DDL (Data Definition Language)
        CREATE,
        ALTER,
        DROP,
        TRUNCATE,
        TABLE,
        INDEX,
        VIEW,

        // Constraints
        PRIMARY,
        KEY,
        FOREIGN,
        UNIQUE,
        CHECK,
        DEFAULT,

        // Transactions
        COMMIT,
        ROLLBACK,
        BEGIN,
        TRANSACTION,

        // Others
        UNION,
        ALL,
        DISTINCT,
        AS,
        ON,
        WITH;

        @Override
        public String toString() {
            return this.name();
        }
    }

    Keyword(String val, int pos){
        super(val, pos);
        if (val != null){
            try{
                SQLKeyword v = SQLKeyword.valueOf(val);
                super.value = v;
                super.position = pos;

            } catch (IllegalArgumentException e){
                System.out.println("UNEXPECTED ARGUMENT AT POSITION".concat(String.valueOf(pos)));
            }

        }
    }
}

class Operator extends Token{
    Operator(String val, int pos){
        super(val,pos);
    }
}

class Identifier extends Token {
    Identifier(String val, int pos) {
        super(val, pos);
    }
}
class Number extends Token {
    Number(int val, int pos) {
        super(val, pos);
    }
}
class Boolean extends Token {
    Boolean(boolean val, int pos) {
        super(val, pos);
    }
}
