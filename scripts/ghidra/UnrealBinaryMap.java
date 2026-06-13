/* ###
 * Export a clean-room binary map for a large native executable.
 *
 * This script intentionally records structure and metadata only. It does not
 * decompile or export function bodies.
 */
//@category InGen

import java.io.File;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.AddressSetView;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

public class UnrealBinaryMap extends GhidraScript {
	private static final int DEFAULT_LIMIT = 2000;

	@Override
	public void run() throws Exception {
		String[] args = getScriptArgs();
		if (args.length < 1) {
			printerr("usage: UnrealBinaryMap <output-json> [limit]");
			return;
		}

		File output = new File(args[0]);
		int limit = args.length >= 2 ? Integer.parseInt(args[1]) : DEFAULT_LIMIT;

		int functionCount = 0;
		List<String> functions = new ArrayList<>();
		FunctionIterator functionIterator = currentProgram.getListing().getFunctions(true);
		while (functionIterator.hasNext() && !monitor.isCancelled()) {
			Function function = functionIterator.next();
			functionCount++;
			if (functions.size() < limit) {
				functions.add(jsonObject(
					jsonPair("name", function.getName()),
					jsonPair("entry", function.getEntryPoint().toString()),
					jsonPair("namespace", function.getParentNamespace().getName(true))));
			}
		}

		int externalSymbolCount = 0;
		List<String> externalSymbols = new ArrayList<>();
		SymbolIterator symbolIterator = currentProgram.getSymbolTable().getExternalSymbols();
		while (symbolIterator.hasNext() && !monitor.isCancelled()) {
			Symbol symbol = symbolIterator.next();
			externalSymbolCount++;
			if (externalSymbols.size() < limit) {
				externalSymbols.add(jsonObject(
					jsonPair("name", symbol.getName(true)),
					jsonPair("address", symbol.getAddress().toString())));
			}
		}

		int stringCount = 0;
		List<String> strings = new ArrayList<>();
		DataIterator dataIterator = currentProgram.getListing().getDefinedData(true);
		while (dataIterator.hasNext() && !monitor.isCancelled()) {
			Data data = dataIterator.next();
			String type = data.getDataType().getName().toLowerCase();
			if (type.contains("unicode") || type.contains("string")) {
				String value = data.getDefaultValueRepresentation();
				if (value.length() > 4) {
					stringCount++;
					if (strings.size() < limit) {
						strings.add(jsonObject(
							jsonPair("address", data.getAddress().toString()),
							jsonPair("value", value)));
					}
				}
			}
		}

		long executableBytes = currentProgram.getMemory().getExecuteSet().getNumAddresses();
		long analyzedExecutableBytes = countAnalyzedExecutableBytes();

		try (PrintWriter writer =
			new PrintWriter(output, StandardCharsets.UTF_8.name())) {
			writer.println("{");
			writer.println("  \"program\": " + quote(currentProgram.getName()) + ",");
			writer.println("  \"language\": " + quote(currentProgram.getLanguageID().toString()) + ",");
			writer.println("  \"compiler\": " + quote(currentProgram.getCompilerSpec().getCompilerSpecID().toString()) + ",");
			writer.println("  \"imageBase\": " + quote(currentProgram.getImageBase().toString()) + ",");
			writer.println("  \"executableBytes\": " + executableBytes + ",");
			writer.println("  \"analyzedExecutableBytes\": " + analyzedExecutableBytes + ",");
			writer.println("  \"disassemblyCoverage\": " +
				(executableBytes == 0 ? "0" : String.format(java.util.Locale.ROOT, "%.6f", (double) analyzedExecutableBytes / (double) executableBytes)) + ",");
			writer.println("  \"functionCount\": " + functionCount + ",");
			writer.println("  \"externalSymbolCount\": " + externalSymbolCount + ",");
			writer.println("  \"stringCount\": " + stringCount + ",");
			writer.println("  \"memoryBlocks\": [");
			MemoryBlock[] blocks = currentProgram.getMemory().getBlocks();
			for (int i = 0; i < blocks.length; i++) {
				MemoryBlock block = blocks[i];
				writer.print("    " + jsonObject(
					jsonPair("name", block.getName()),
					jsonPair("start", block.getStart().toString()),
					jsonPair("end", block.getEnd().toString()),
					jsonPair("size", Long.toString(block.getSize()), false),
					jsonPair("read", Boolean.toString(block.isRead()), false),
					jsonPair("write", Boolean.toString(block.isWrite()), false),
					jsonPair("execute", Boolean.toString(block.isExecute()), false)));
				writer.println(i + 1 == blocks.length ? "" : ",");
			}
			writer.println("  ],");
			writer.println("  \"sampledFunctions\": [" + String.join(",", functions) + "],");
			writer.println("  \"sampledExternalSymbols\": [" + String.join(",", externalSymbols) + "],");
			writer.println("  \"sampledStrings\": [" + String.join(",", strings) + "]");
			writer.println("}");
		}

		println("Wrote clean-room binary map to " + output.getAbsolutePath());
	}

	private long countAnalyzedExecutableBytes() {
		AddressSetView executeSet = currentProgram.getMemory().getExecuteSet();
		long analyzed = 0;
		InstructionIterator instructions =
			currentProgram.getListing().getInstructions(executeSet, true);
		while (instructions.hasNext()) {
			Instruction instruction = instructions.next();
			analyzed += instruction.getLength();
		}
		DataIterator data = currentProgram.getListing().getData(executeSet, true);
		while (data.hasNext()) {
			Data item = data.next();
			if (item.isDefined()) {
				analyzed += item.getLength();
			}
		}
		return analyzed;
	}

	private static String jsonObject(String... pairs) {
		return "{" + String.join(",", pairs) + "}";
	}

	private static String jsonPair(String key, String value) {
		return jsonPair(key, value, true);
	}

	private static String jsonPair(String key, String value, boolean quoteValue) {
		return quote(key) + ":" + (quoteValue ? quote(value) : value);
	}

	private static String quote(String value) {
		if (value == null) {
			return "null";
		}
		StringBuilder builder = new StringBuilder();
		builder.append('"');
		for (int i = 0; i < value.length(); i++) {
			char c = value.charAt(i);
			switch (c) {
				case '\\':
					builder.append("\\\\");
					break;
				case '"':
					builder.append("\\\"");
					break;
				case '\n':
					builder.append("\\n");
					break;
				case '\r':
					builder.append("\\r");
					break;
				case '\t':
					builder.append("\\t");
					break;
				default:
					if (c < 0x20) {
						builder.append(String.format("\\u%04x", (int) c));
					}
					else {
						builder.append(c);
					}
					break;
			}
		}
		builder.append('"');
		return builder.toString();
	}
}
